// check-pass
#![feature(generic_const_exprs, adt_const_params)]
#![allow(incomplete_features)]

#[derive(PartialEq, Eq)]
enum MyEnum {
    Var(u32),
}

struct HasMyEnum<const N: MyEnum>;

fn foo<const N: u32>(a: HasMyEnum<{ MyEnum::Var(N) }>) {
    bar::<{ MyEnum::Var { 0: N } }>();
    bar::<{ MyEnum::Var(N) }>();
}

fn bar<const N: MyEnum>() {}

fn main() {}
