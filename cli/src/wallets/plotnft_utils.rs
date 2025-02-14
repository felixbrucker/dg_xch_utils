use crate::wallets::common::sign_coin_spend;
use crate::wallets::memory_wallet::{MemoryWalletConfig, MemoryWalletStore};
use crate::wallets::{Wallet, WalletInfo};
use async_trait::async_trait;
use blst::min_pk::SecretKey;
use dg_xch_clients::api::full_node::FullnodeAPI;
use dg_xch_clients::protocols::pool::{FARMING_TO_POOL, LEAVING_POOL, POOL_PROTOCOL_VERSION};
use dg_xch_clients::rpc::full_node::FullnodeClient;
use dg_xch_core::blockchain::announcement::Announcement;
use dg_xch_core::blockchain::coin_record::CoinRecord;
use dg_xch_core::blockchain::coin_spend::{compute_additions_with_cost, CoinSpend};
use dg_xch_core::blockchain::sized_bytes::{Bytes32, Bytes48};
use dg_xch_core::blockchain::spend_bundle::SpendBundle;
use dg_xch_core::blockchain::transaction_record::{TransactionRecord, TransactionType};
use dg_xch_core::blockchain::tx_status::TXStatus;
use dg_xch_core::blockchain::wallet_type::WalletType;
use dg_xch_core::consensus::constants::ConsensusConstants;
use dg_xch_core::plots::PlotNft;
use dg_xch_core::pool::PoolState;
use dg_xch_keys::{
    master_sk_to_singleton_owner_sk, master_sk_to_wallet_sk, master_sk_to_wallet_sk_unhardened,
};
use dg_xch_puzzles::clvm_puzzles::{
    create_full_puzzle, create_travel_spend, get_most_recent_singleton_coin_from_coin_spend,
    launcher_coin_spend_to_extra_data, pool_state_to_inner_puzzle, solution_to_pool_state,
    SINGLETON_LAUNCHER_HASH,
};
use dg_xch_puzzles::p2_delegated_puzzle_or_hidden_puzzle::puzzle_hash_for_pk;
use log::info;
use num_traits::cast::ToPrimitive;
use std::future::Future;
use std::io::{Error, ErrorKind};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;

pub struct PlotNFTWallet {
    info: WalletInfo<MemoryWalletStore>,
    pub config: MemoryWalletConfig,
    fullnode_client: FullnodeClient,
}
#[async_trait]
impl Wallet<MemoryWalletStore, MemoryWalletConfig> for PlotNFTWallet {
    fn create(info: WalletInfo<MemoryWalletStore>, config: MemoryWalletConfig) -> Self {
        Self {
            fullnode_client: FullnodeClient::new(
                &config.fullnode_host,
                config.fullnode_port,
                config.fullnode_ssl_path.clone(),
                &config.additional_headers,
            ),
            info,
            config,
        }
    }

    fn name(&self) -> &str {
        &self.info.name
    }

    async fn sync(&self) -> Result<bool, Error> {
        let mut puzzle_hashes = vec![];
        for index in 0..50 {
            let wallet_sk = master_sk_to_wallet_sk(&self.info.master_sk, index).map_err(|e| {
                Error::new(
                    ErrorKind::InvalidInput,
                    format!("Failed to parse Wallet SK: {:?}", e),
                )
            })?;
            let pub_key: Bytes48 = wallet_sk.sk_to_pk().to_bytes().into();
            let ph = puzzle_hash_for_pk(&pub_key)?;
            puzzle_hashes.push(ph);
            let wallet_sk = master_sk_to_wallet_sk_unhardened(&self.info.master_sk, index)
                .map_err(|e| {
                    Error::new(
                        ErrorKind::InvalidInput,
                        format!("Failed to parse Wallet SK: {:?}", e),
                    )
                })?;
            let pub_key: Bytes48 = wallet_sk.sk_to_pk().to_bytes().into();
            let ph = puzzle_hash_for_pk(&pub_key)?;
            puzzle_hashes.push(ph);
        }
        let (spend, unspent) =
            scrounge_for_standard_coins(&self.fullnode_client, &puzzle_hashes).await?;
        let mut store = self.info.wallet_store.lock().await;
        store.spent_coins.clear();
        store.unspent_coins.clear();
        store
            .spent_coins
            .extend(spend.into_iter().map(|v| (v.coin.name(), v)));
        store
            .unspent_coins
            .extend(unspent.into_iter().map(|v| (v.coin.name(), v)));
        Ok(true)
    }

    fn is_synced(&self) -> bool {
        todo!()
    }

    fn wallet_info(&self) -> &WalletInfo<MemoryWalletStore> {
        &self.info
    }

    fn wallet_store(&self) -> Arc<Mutex<MemoryWalletStore>> {
        self.info.wallet_store.clone()
    }
}
impl PlotNFTWallet {
    pub fn new(master_secret_key: SecretKey, client: &FullnodeClient) -> Self {
        Self::create(
            WalletInfo {
                id: 1,
                name: "pooling_wallet".to_string(),
                wallet_type: WalletType::PoolingWallet,
                constants: Default::default(),
                master_sk: master_secret_key.clone(),
                wallet_store: Arc::new(Mutex::new(MemoryWalletStore::new(master_secret_key, 0))),
                data: "".to_string(),
            },
            MemoryWalletConfig {
                fullnode_host: client.host.clone(),
                fullnode_port: client.port,
                fullnode_ssl_path: client.ssl_path.clone(),
                additional_headers: client.additional_headers.clone(),
            },
        )
    }
    pub fn find_owner_key(&self, key_to_find: &Bytes48, limit: u32) -> Result<SecretKey, Error> {
        for i in 0..limit {
            let key = master_sk_to_singleton_owner_sk(&self.wallet_info().master_sk, i)?;
            if &key.sk_to_pk().to_bytes() == key_to_find.to_sized_bytes() {
                return Ok(key);
            }
        }
        Err(Error::new(ErrorKind::NotFound, "Failed to find Owner SK"))
    }

    pub async fn generate_fee_transaction(
        &self,
        fee: u64,
        coin_announcements: Option<&[Announcement]>,
    ) -> Result<TransactionRecord, Error> {
        self.generate_signed_transaction(
            0,
            &self.get_new_puzzlehash().await?,
            fee,
            None,
            None,
            None,
            false,
            coin_announcements,
            None,
            None,
            false,
            None,
            None,
            None,
            None,
            None,
        )
        .await
    }

    pub async fn generate_travel_transaction(
        &self,
        plot_nft: &PlotNft,
        target_pool_state: &PoolState,
        fee: u64,
        constants: &ConsensusConstants,
    ) -> Result<(TransactionRecord, Option<TransactionRecord>), Error> {
        let launcher_coin = self
            .fullnode_client
            .get_coin_record_by_name(&plot_nft.launcher_id)
            .await?
            .ok_or_else(|| Error::new(ErrorKind::Other, "Failed to load launcher_coin"))?;
        let last_record = self
            .fullnode_client
            .get_coin_record_by_name(&plot_nft.singleton_coin.coin.parent_coin_info)
            .await?
            .ok_or_else(|| Error::new(ErrorKind::Other, "Failed to load launcher_coin"))?;
        let last_coin_spend = self.fullnode_client.get_coin_spend(&last_record).await?;
        let next_state = if plot_nft.pool_state.state == FARMING_TO_POOL {
            PoolState {
                version: POOL_PROTOCOL_VERSION,
                state: LEAVING_POOL,
                target_puzzle_hash: plot_nft.pool_state.target_puzzle_hash,
                owner_pubkey: plot_nft.pool_state.owner_pubkey,
                pool_url: plot_nft.pool_state.pool_url.clone(),
                relative_lock_height: plot_nft.pool_state.relative_lock_height,
            }
        } else {
            target_pool_state.clone()
        };
        let new_inner_puzzle = pool_state_to_inner_puzzle(
            &next_state,
            &launcher_coin.coin.name(),
            &constants.genesis_challenge,
            plot_nft.delay_time as u64,
            &plot_nft.delay_puzzle_hash,
        )?;
        let new_full_puzzle = create_full_puzzle(&new_inner_puzzle, &launcher_coin.coin.name())?;
        let (outgoing_coin_spend, inner_puzzle) = create_travel_spend(
            &last_coin_spend,
            &launcher_coin.coin,
            &plot_nft.pool_state,
            &next_state,
            &constants.genesis_challenge,
            plot_nft.delay_time as u64,
            &plot_nft.delay_puzzle_hash,
        )?;
        let (additions, _cost) = compute_additions_with_cost(
            &last_coin_spend,
            constants.max_block_cost_clvm.to_u64().unwrap(),
        )?;
        let singleton = &additions[0];
        let singleton_id = singleton.name();
        assert_eq!(
            outgoing_coin_spend.coin.parent_coin_info,
            last_coin_spend.coin.name()
        );
        assert_eq!(
            outgoing_coin_spend.coin.parent_coin_info,
            last_coin_spend.coin.name()
        );
        assert_eq!(outgoing_coin_spend.coin.name(), singleton_id);
        assert_ne!(new_inner_puzzle, inner_puzzle);
        let mut signed_spend_bundle = sign_coin_spend(
            outgoing_coin_spend,
            |_| async { self.find_owner_key(&plot_nft.pool_state.owner_pubkey, 500) },
            constants,
        )
        .await?;
        assert_eq!(
            signed_spend_bundle.removals()[0].puzzle_hash,
            singleton.puzzle_hash
        );
        assert_eq!(signed_spend_bundle.removals()[0].name(), singleton.name());
        let fee_tx: Option<TransactionRecord> = None;
        if fee > 0 {
            let fee_tx = self.generate_fee_transaction(fee, None).await?;
            if let Some(fee_bundle) = fee_tx.spend_bundle {
                signed_spend_bundle = SpendBundle::aggregate(vec![signed_spend_bundle, fee_bundle])
                    .map_err(|e| {
                        Error::new(
                            ErrorKind::Other,
                            format!("Failed to parse Public key: {:?}", e),
                        )
                    })?;
            }
        }
        let additions = signed_spend_bundle.additions()?;
        let removals = signed_spend_bundle.removals();
        let name = signed_spend_bundle.name();
        let tx_record = TransactionRecord {
            confirmed_at_height: 0,
            created_at_time: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            to_puzzle_hash: new_full_puzzle.tree_hash(),
            amount: 1,
            fee_amount: fee,
            confirmed: false,
            sent: 0,
            spend_bundle: Some(signed_spend_bundle),
            additions,
            removals,
            wallet_id: 1,
            sent_to: vec![],
            trade_id: None,
            memos: vec![],
            transaction_type: TransactionType::OutgoingTx as u32,
            name,
        };
        Ok((tx_record, fee_tx))
    }
}

pub async fn generate_travel_transaction_without_fee<F, Fut>(
    client: &FullnodeClient,
    key_fn: F,
    plot_nft: &PlotNft,
    target_pool_state: &PoolState,
    constants: &ConsensusConstants,
) -> Result<(TransactionRecord, Option<TransactionRecord>), Error>
where
    F: Fn(&Bytes48) -> Fut,
    Fut: Future<Output = Result<SecretKey, Error>>,
{
    let launcher_coin = client
        .get_coin_record_by_name(&plot_nft.launcher_id)
        .await?
        .ok_or_else(|| Error::new(ErrorKind::Other, "Failed to load launcher_coin"))?;
    let last_record = client
        .get_coin_record_by_name(&plot_nft.singleton_coin.coin.parent_coin_info)
        .await?
        .ok_or_else(|| Error::new(ErrorKind::Other, "Failed to load launcher_coin"))?;
    let last_coin_spend = client.get_coin_spend(&last_record).await?;
    let next_state = if plot_nft.pool_state.state == FARMING_TO_POOL {
        PoolState {
            version: POOL_PROTOCOL_VERSION,
            state: LEAVING_POOL,
            target_puzzle_hash: plot_nft.pool_state.target_puzzle_hash,
            owner_pubkey: plot_nft.pool_state.owner_pubkey,
            pool_url: plot_nft.pool_state.pool_url.clone(),
            relative_lock_height: plot_nft.pool_state.relative_lock_height,
        }
    } else {
        target_pool_state.clone()
    };
    let new_inner_puzzle = pool_state_to_inner_puzzle(
        &next_state,
        &launcher_coin.coin.name(),
        &constants.genesis_challenge,
        plot_nft.delay_time as u64,
        &plot_nft.delay_puzzle_hash,
    )?;
    let new_full_puzzle = create_full_puzzle(&new_inner_puzzle, &launcher_coin.coin.name())?;
    let (outgoing_coin_spend, inner_puzzle) = create_travel_spend(
        &last_coin_spend,
        &launcher_coin.coin,
        &plot_nft.pool_state,
        &next_state,
        &constants.genesis_challenge,
        plot_nft.delay_time as u64,
        &plot_nft.delay_puzzle_hash,
    )?;
    let (additions, _cost) = compute_additions_with_cost(
        &last_coin_spend,
        constants.max_block_cost_clvm.to_u64().unwrap(),
    )?;
    let singleton = &additions[0];
    let singleton_id = singleton.name();
    assert_eq!(
        outgoing_coin_spend.coin.parent_coin_info,
        last_coin_spend.coin.name()
    );
    assert_eq!(
        outgoing_coin_spend.coin.parent_coin_info,
        last_coin_spend.coin.name()
    );
    assert_eq!(outgoing_coin_spend.coin.name(), singleton_id);
    assert_ne!(new_inner_puzzle, inner_puzzle);
    let signed_spend_bundle = sign_coin_spend(outgoing_coin_spend, key_fn, constants).await?;
    assert_eq!(
        signed_spend_bundle.removals()[0].puzzle_hash,
        singleton.puzzle_hash
    );
    assert_eq!(signed_spend_bundle.removals()[0].name(), singleton.name());
    let additions = signed_spend_bundle.additions()?;
    let removals = signed_spend_bundle.removals();
    let name = signed_spend_bundle.name();
    let tx_record = TransactionRecord {
        confirmed_at_height: 0,
        created_at_time: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        to_puzzle_hash: new_full_puzzle.tree_hash(),
        amount: 1,
        fee_amount: 0,
        confirmed: false,
        sent: 0,
        spend_bundle: Some(signed_spend_bundle),
        additions,
        removals,
        wallet_id: 1,
        sent_to: vec![],
        trade_id: None,
        memos: vec![],
        transaction_type: TransactionType::OutgoingTx as u32,
        name,
    };
    Ok((tx_record, None))
}

pub async fn get_current_pool_state(
    client: &FullnodeClient,
    launcher_id: &Bytes32,
) -> Result<(PoolState, CoinSpend), Error> {
    let mut last_spend: CoinSpend;
    let mut saved_state: PoolState;
    match client.get_coin_record_by_name(launcher_id).await? {
        Some(lc) if lc.spent => {
            last_spend = client.get_coin_spend(&lc).await?;
            match solution_to_pool_state(&last_spend)? {
                Some(state) => {
                    saved_state = state;
                }
                None => {
                    return Err(Error::new(
                        ErrorKind::InvalidData,
                        "Failed to Read Pool State",
                    ));
                }
            }
        }
        Some(_) => {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!("Genesis coin {} not spent", &launcher_id.to_string()),
            ));
        }
        None => {
            return Err(Error::new(
                ErrorKind::NotFound,
                format!("Can not find genesis coin {}", &launcher_id),
            ));
        }
    }
    let mut saved_spend: CoinSpend = last_spend.clone();
    let mut last_not_none_state: PoolState = saved_state.clone();
    loop {
        match get_most_recent_singleton_coin_from_coin_spend(&last_spend)? {
            None => {
                return Err(Error::new(
                    ErrorKind::NotFound,
                    "Failed to find recent singleton from coin Record",
                ));
            }
            Some(next_coin) => match client.get_coin_record_by_name(&next_coin.name()).await? {
                None => {
                    return Err(Error::new(
                        ErrorKind::NotFound,
                        "Failed to find Coin Record",
                    ));
                }
                Some(next_coin_record) => {
                    if !next_coin_record.spent {
                        break;
                    }
                    last_spend = client.get_coin_spend(&next_coin_record).await?;
                    if let Ok(Some(pool_state)) = solution_to_pool_state(&last_spend) {
                        last_not_none_state = pool_state;
                    }
                    saved_spend = last_spend.clone();
                    saved_state = last_not_none_state.clone();
                }
            },
        }
    }
    Ok((saved_state, saved_spend))
}

pub async fn scrounge_for_plotnft_by_key(
    client: &FullnodeClient,
    master_secret_key: &SecretKey,
) -> Result<Vec<PlotNft>, Error> {
    let mut page = 0;
    let mut plotnfs = vec![];
    while page < 15 && plotnfs.is_empty() {
        let mut puzzle_hashes = vec![];
        for index in page * 50..(page + 1) * 50 {
            let wallet_sk =
                master_sk_to_wallet_sk_unhardened(master_secret_key, index).map_err(|e| {
                    Error::new(
                        ErrorKind::InvalidInput,
                        format!("Failed to parse Wallet SK: {:?}", e),
                    )
                })?;
            let pub_key: Bytes48 = wallet_sk.sk_to_pk().to_bytes().into();
            let ph = puzzle_hash_for_pk(&pub_key)?;
            puzzle_hashes.push(ph);
        }
        plotnfs.extend(scrounge_for_plotnfts(client, &puzzle_hashes).await?);
        page += 1;
    }
    Ok(plotnfs)
}

pub async fn scrounge_for_plotnfts(
    client: &FullnodeClient,
    puzzle_hashes: &[Bytes32],
) -> Result<Vec<PlotNft>, Error> {
    let hashes = client
        .get_coin_records_by_puzzle_hashes(puzzle_hashes, Some(true), None, None)
        .await?;
    let spent: Vec<CoinRecord> = hashes.into_iter().filter(|c| c.spent).collect();
    let mut plotnfts = vec![];
    for spent_coin in spent {
        let coin_spend = client.get_coin_spend(&spent_coin).await?;
        for child in coin_spend.additions()? {
            if child.puzzle_hash == *SINGLETON_LAUNCHER_HASH {
                let launcher_id = child.name();
                if let Some(plotnft) = get_plotnft_by_launcher_id(client, &launcher_id).await? {
                    plotnfts.push(plotnft);
                }
            }
        }
    }
    Ok(plotnfts)
}

pub async fn scrounge_for_standard_coins(
    client: &FullnodeClient,
    puzzle_hashes: &[Bytes32],
) -> Result<(Vec<CoinRecord>, Vec<CoinRecord>), Error> {
    let records = client
        .get_coin_records_by_puzzle_hashes(puzzle_hashes, Some(true), None, None)
        .await?;
    let mut spent = vec![];
    let mut unspent = vec![];
    for coin in records {
        if coin.spent {
            spent.push(coin);
        } else {
            unspent.push(coin);
        }
    }
    Ok((spent, unspent))
}

pub async fn get_pool_state(
    client: &FullnodeClient,
    launcher_id: &Bytes32,
) -> Result<PoolState, Error> {
    if let Some(plotnft) = get_plotnft_by_launcher_id(client, launcher_id).await? {
        Ok(plotnft.pool_state)
    } else {
        Err(Error::new(
            ErrorKind::NotFound,
            format!("Failed to find pool state for launcher_id {}", launcher_id),
        ))
    }
}

pub async fn get_plotnft_by_launcher_id(
    client: &FullnodeClient,
    launcher_id: &Bytes32,
) -> Result<Option<PlotNft>, Error> {
    let launcher_coin = client.get_coin_record_by_name(launcher_id).await?;
    if let Some(launcher_coin) = launcher_coin {
        let spend = client.get_coin_spend(&launcher_coin).await?;
        let initial_extra_data = launcher_coin_spend_to_extra_data(&spend)?;
        let first_coin = get_most_recent_singleton_coin_from_coin_spend(&spend)?;
        if let Some(coin) = first_coin {
            let mut last_not_null_state = initial_extra_data.pool_state.clone();
            let mut singleton_coin = client.get_coin_record_by_name(&coin.name()).await?;
            while let Some(sc) = &singleton_coin {
                if sc.spent {
                    let last_spend = client.get_coin_spend(sc).await?;
                    let next_coin = get_most_recent_singleton_coin_from_coin_spend(&last_spend)?;
                    if let Some(pool_state) = solution_to_pool_state(&last_spend)? {
                        last_not_null_state = pool_state;
                    }
                    if let Some(nc) = next_coin {
                        singleton_coin = client.get_coin_record_by_name(&nc.name()).await?;
                    } else {
                        break; //Error?
                    }
                } else {
                    break;
                }
            }
            if let Some(singleton_coin) = singleton_coin {
                Ok(Some(PlotNft {
                    launcher_id: *launcher_id,
                    singleton_coin,
                    pool_state: last_not_null_state,
                    delay_time: initial_extra_data.delay_time,
                    delay_puzzle_hash: initial_extra_data.delay_puzzle_hash,
                }))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    } else {
        Ok(None)
    }
}

pub async fn submit_next_state_spend_bundle(
    client: &FullnodeClient,
    pool_wallet: &PlotNFTWallet,
    plot_nft: &PlotNft,
    target_pool_state: &PoolState,
    fee: u64,
) -> Result<(), Error> {
    let (travel_record, _) = pool_wallet
        .generate_travel_transaction(
            plot_nft,
            target_pool_state,
            fee,
            &pool_wallet.info.constants,
        )
        .await?;
    let coin_to_find = travel_record
        .additions
        .iter()
        .find(|c| c.amount == 1)
        .expect("Failed to find NFT coin");
    match client
        .push_tx(
            &travel_record
                .spend_bundle
                .expect("Expected Transaction Record to have Spend bundle"),
        )
        .await?
    {
        TXStatus::SUCCESS => {
            info!("Transaction Submitted Successfully. Waiting for coin to show as spent...");
            loop {
                if let Ok(Some(record)) = client.get_coin_record_by_name(&coin_to_find.name()).await
                {
                    if let Ok(Some(record)) = client
                        .get_coin_record_by_name(&record.coin.parent_coin_info)
                        .await
                    {
                        info!(
                            "Found spent parent coin, Parent Coin was spent at {}",
                            record.spent_block_index
                        );
                        break;
                    }
                }
                tokio::time::sleep(Duration::from_secs(10)).await;
                info!("Waiting for plot_nft spend to appear...");
            }
            Ok(())
        }
        TXStatus::PENDING => Err(Error::new(ErrorKind::Other, "Transaction is pending")),
        TXStatus::FAILED => Err(Error::new(ErrorKind::Other, "Failed to submit transaction")),
    }
}

pub async fn submit_next_state_spend_bundle_with_key(
    client: &FullnodeClient,
    secret_key: &SecretKey,
    plot_nft: &PlotNft,
    target_pool_state: &PoolState,
    constants: &ConsensusConstants,
) -> Result<(), Error> {
    let (travel_record, _) = generate_travel_transaction_without_fee(
        client,
        |_| async { Ok(secret_key.clone()) },
        plot_nft,
        target_pool_state,
        constants,
    )
    .await?;
    let coin_to_find = travel_record
        .additions
        .iter()
        .find(|c| c.amount == 1)
        .expect("Failed to find NFT coin");
    match client
        .push_tx(
            &travel_record
                .spend_bundle
                .expect("Expected Transaction Record to have Spend bundle"),
        )
        .await?
    {
        TXStatus::SUCCESS => {
            info!("Transaction Submitted Successfully. Waiting for coin to show as spent...");
            loop {
                if let Ok(Some(record)) = client.get_coin_record_by_name(&coin_to_find.name()).await
                {
                    if let Ok(Some(record)) = client
                        .get_coin_record_by_name(&record.coin.parent_coin_info)
                        .await
                    {
                        info!(
                            "Found spent parent coin, Parent Coin was spent at {}",
                            record.spent_block_index
                        );
                        break;
                    }
                }
                tokio::time::sleep(Duration::from_secs(10)).await;
                info!("Waiting for plot_nft spend to appear...");
            }
            Ok(())
        }
        TXStatus::PENDING => Err(Error::new(ErrorKind::Other, "Transaction is pending")),
        TXStatus::FAILED => Err(Error::new(ErrorKind::Other, "Failed to submit transaction")),
    }
}
