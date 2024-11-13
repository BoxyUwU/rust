//@ revisions: full min
#![cfg_attr(full, feature(adt_const_params))]
#![cfg_attr(full, allow(incomplete_features))]

fn foo<const LEN: usize, const DATA: [u8; LEN]>() {}
//~^ ERROR the type of const parameters must not
fn main() {
    const DATA: [u8; 4] = *b"ABCD";
    foo::<4, DATA>();
}
