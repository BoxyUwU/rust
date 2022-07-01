// check-pass

#![feature(generic_const_exprs)]
#![allow(incomplete_features)]

use std::marker::PhantomData;

struct Nil;
struct Cons<N>(PhantomData<N>);

trait Len {
    const N: usize;
}

impl<N: Len> Len for Cons<N> {
    const N: usize = 1 + N::N;
}

impl Len for Nil {
    const N: usize = 0;
}

trait Piece<T> {
    type Size: Len;

    fn construct(self) -> [T; Self::Size::N];
}

impl<T: Copy + From<i32>> Piece<T> for i32 {
    type Size = Cons<Nil>;

    fn construct(self) -> [T; Self::Size::N] {
        [T::from(self)]
    }
}

impl<T: Copy, U: Piece<T>> Piece<T> for (U,) {
    type Size = U::Size;

    fn construct(self) -> [T; Self::Size::N] {
        todo!()
    }
}

fn main() {}
