//@ revisions: full min
#![cfg_attr(full, feature(adt_const_params, generic_arg_infer))]
#![cfg_attr(full, allow(incomplete_features))]

fn foo<const N: usize, const A: [u8; N]>() {}
//~^ ERROR the type of const parameters must not

fn main() {
    foo::<_, { [1] }>();
    //[min]~^ ERROR: type provided when a constant was expected
}
