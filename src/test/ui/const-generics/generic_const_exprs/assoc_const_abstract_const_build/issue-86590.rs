// check-pass

#![allow(incomplete_features)]
#![feature(generic_const_exprs)]

trait Hmm {
    const W: usize;
    fn hmm(&self, res: &[u8; Self::W]);
}

impl<X: Hmm> Hmm for Option<X> {
    const W: usize = X::W;
    fn hmm(&self, res: &[u8; Self::W]) {
        match self {
            Some(x) => x.hmm(&res),
            None => todo!(),
        }
    }
}

fn main() {}
