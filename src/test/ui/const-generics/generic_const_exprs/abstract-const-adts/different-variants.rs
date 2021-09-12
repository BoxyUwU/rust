#![feature(generic_const_exprs, adt_const_params)]
#![allow(incomplete_features)]

#[derive(PartialEq, Eq)]
enum MyEnum {
    Var1 { field: u32 },
    Var2 { field: u32 },
}

struct HasMyEnum<const N: MyEnum>;

fn foo<const N: u32>(a: HasMyEnum<{ MyEnum::Var1 { field: N } }>) {
    bar::<{ MyEnum::Var1 { field: N } }>();
    bar::<{ MyEnum::Var2 { field: N } }>();
    //~^ error: unconstrained generic constant
}

fn bar<const N: MyEnum>() {}

fn main() {}
