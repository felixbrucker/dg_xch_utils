use lazy_static::lazy_static;
use std::collections::HashMap;
use std::io::{Error, ErrorKind};

lazy_static! {
    pub static ref NULL: SExp = SExp::Atom(vec![].into());
    pub static ref ONE: SExp = SExp::Atom(vec![1u8].into());
}

#[derive(Clone, Hash, Debug, PartialEq, Eq)]
pub enum SExp {
    Atom(AtomBuf),
    Pair(PairBuf),
}
impl SExp {
    pub fn atom(&self) -> Result<&AtomBuf, Error> {
        match self {
            SExp::Atom(a) => Ok(a),
            SExp::Pair(_) => Err(Error::new(
                ErrorKind::Unsupported,
                "Expected Atom, got Pair",
            )),
        }
    }
    pub fn pair(&self) -> Result<&PairBuf, Error> {
        match self {
            SExp::Atom(_) => Err(Error::new(
                ErrorKind::Unsupported,
                "Expected Pair, got Atom",
            )),
            SExp::Pair(p) => Ok(p),
        }
    }
    pub fn first(&self) -> Result<&SExp, Error> {
        match self {
            SExp::Atom(_) => Err(Error::new(
                ErrorKind::Unsupported,
                "Expected Pair, got Atom",
            )),
            SExp::Pair(p) => Ok(&p.first),
        }
    }
    pub fn rest(&self) -> Result<&SExp, Error> {
        match self {
            SExp::Atom(_) => Err(Error::new(
                ErrorKind::Unsupported,
                "Expected Pair, got Atom",
            )),
            SExp::Pair(p) => Ok(&p.rest),
        }
    }
    pub fn cons(self, other: SExp) -> Result<SExp, Error> {
        Ok(SExp::Pair(PairBuf {
            first: Box::new(self),
            rest: Box::new(other),
        }))
    }
    pub fn split(self) -> Result<(SExp, SExp), Error> {
        match self {
            SExp::Atom(_) => Err(Error::new(
                ErrorKind::Unsupported,
                "Expected Pair, got Atom",
            )),
            SExp::Pair(p) => Ok((*p.first, *p.rest)),
        }
    }

    pub fn nullp(&self) -> bool {
        match &self {
            SExp::Atom(a) => a.data.is_empty(),
            _ => false,
        }
    }

    pub fn as_atom_list(&self) -> Vec<Vec<u8>> {
        match self {
            SExp::Atom(_) => {
                vec![]
            }
            SExp::Pair(pair) => match pair.first.as_ref() {
                SExp::Atom(buf) => {
                    let mut rtn: Vec<Vec<u8>> = vec![buf.data.clone()];
                    rtn.extend(pair.rest.as_atom_list());
                    rtn
                }
                SExp::Pair(_) => {
                    vec![]
                }
            },
        }
    }

    pub fn to_map(self) -> Result<HashMap<SExp, SExp>, Error> {
        let mut rtn: HashMap<SExp, SExp> = HashMap::new();
        let mut cur_node = self;
        loop {
            match cur_node {
                SExp::Atom(_) => break,
                SExp::Pair(pair) => {
                    cur_node = *pair.rest;
                    match *pair.first {
                        SExp::Atom(_) => {
                            rtn.insert(*pair.first, NULL.clone());
                        }
                        SExp::Pair(inner_pair) => {
                            rtn.insert(*inner_pair.first, *inner_pair.rest);
                        }
                    }
                }
            }
        }
        Ok(rtn)
    }

    pub fn arg_count_is(&self, mut count: usize) -> bool {
        let mut ptr = self;
        loop {
            if count == 0 {
                return ptr.nullp();
            }
            match ptr {
                SExp::Pair(pair) => {
                    ptr = &pair.rest;
                }
                _ => return false,
            }
            count -= 1;
        }
    }

    pub fn iter(&self) -> SExpIter {
        SExpIter { c: self }
    }

    pub fn as_bool(&self) -> bool {
        match self.atom() {
            Ok(v0) => !v0.data.is_empty(),
            _ => true,
        }
    }

    pub fn from_bool(b: bool) -> &'static SExp {
        if b {
            &ONE
        } else {
            &NULL
        }
    }

    pub fn proper_list(self, store: bool) -> Option<Vec<SExp>> {
        let mut args = vec![];
        let mut args_sexp = self;
        loop {
            match args_sexp {
                SExp::Atom(_) => {
                    return if !args_sexp.non_nil() {
                        Some(args)
                    } else {
                        None
                    };
                }
                SExp::Pair(buf) => {
                    if store {
                        args.push(*buf.first);
                    }
                    args_sexp = *buf.rest;
                }
            }
        }
    }

    pub fn non_nil(&self) -> bool {
        match self {
            SExp::Pair(_) => true,
            SExp::Atom(b) => !b.data.is_empty(),
        }
    }
}

pub struct SExpIter<'a> {
    c: &'a SExp,
}

impl<'a> Iterator for SExpIter<'a> {
    type Item = &'a SExp;

    fn next(&mut self) -> Option<Self::Item> {
        match self.c.pair().ok() {
            Some(pair) => {
                self.c = &pair.rest;
                Some(&pair.first)
            }
            _ => None,
        }
    }
}

#[derive(Clone, Hash, Debug, PartialEq, Eq)]
pub struct AtomBuf {
    pub data: Vec<u8>,
}

impl AtomBuf {
    pub fn new(v: Vec<u8>) -> Self {
        AtomBuf { data: v }
    }
}

impl From<&[u8]> for AtomBuf {
    fn from(v: &[u8]) -> Self {
        Self::new(v.to_vec())
    }
}

impl From<Vec<u8>> for AtomBuf {
    fn from(v: Vec<u8>) -> Self {
        Self::new(v)
    }
}

impl From<&Vec<u8>> for AtomBuf {
    fn from(v: &Vec<u8>) -> Self {
        Self::from(v.clone())
    }
}

#[derive(Clone, Hash, Debug, PartialEq, Eq)]
pub struct PairBuf {
    pub first: Box<SExp>,
    pub rest: Box<SExp>,
}

impl From<(&SExp, &SExp)> for PairBuf {
    fn from(v: (&SExp, &SExp)) -> Self {
        PairBuf {
            first: Box::new(v.0.clone()),
            rest: Box::new(v.1.clone()),
        }
    }
}

impl From<(SExp, SExp)> for PairBuf {
    fn from(v: (SExp, SExp)) -> Self {
        PairBuf {
            first: Box::new(v.0),
            rest: Box::new(v.1),
        }
    }
}
