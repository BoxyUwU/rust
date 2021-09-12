#![feature(generic_const_exprs, adt_const_params)]
#![allow(incomplete_features)]

#[derive(PartialEq, Eq)]
enum Foo<T: Eq + PartialEq> {
    Unused(T),
    Variant(u32),
}

struct HasFooU16<const N: Foo<u16>>;

fn doesnt_unify<const N: u32>(_: HasFooU16<{ Foo::<u16>::Variant(N) }>) {
    bar::<{ Foo::<u8>::Variant(N) }>();
    //~^ error: unconstrained generic constant
}

struct HasFooU8<const N: Foo<u8>>;

fn does_unify<const N: u32>(_: HasFooU8<{ Foo::<u8>::Variant(N) }>) {
    bar::<{ Foo::<u8>::Variant(N) }>();
}

fn bar<const M: Foo<u8>>() {}

fn main() {}
