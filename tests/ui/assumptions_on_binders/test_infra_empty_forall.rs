//@ check-pass
//@ compile-flags: -Zassumptions-on-binders -Znext-solver=globally

#![feature(test_binder_constraints, non_lifetime_binders)]
#![expect(incomplete_features)]

core::test_binder_constraints! {
    impl<'a: 'b, 'b> {
        forall<'c> {
            'a: 'b
        }
    }
}

fn main() {}
