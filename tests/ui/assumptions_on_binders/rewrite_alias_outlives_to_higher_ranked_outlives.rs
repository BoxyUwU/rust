//@ compile-flags: -Zassumptions-on-binders -Znext-solver=globally
//@ check-pass

// test that we can handle something like `Alias<!a_u1>: 'c` by rewriting it to `for<'a> Alias<'a>: 'c`
// then proving it via a higher ranked assumption from a smaller universe.

#![feature(test_binder_constraints, generic_const_items)]

trait Trait<'a, 'b> {
    type Assoc;
}

core::test_binder_constraints! {
    impl<'c, T> 
    where
        T: for<'r1, 'r2> Trait<'r1, 'r2>,
    {
        forall<'a>
        where
            for<'d> <T as Trait<'a, 'd>>::Assoc: 'c,
        {
            forall<'b> {
                where <T as Trait<'a, 'b>>::Assoc: 'c
            } expect {
                for<'b> <T as Trait<'a, 'b>>::Assoc: 'c
            }
        // check that we match the above constraint to the assumption on this forall rather than
        // propagating out a `for<'a, 'b> <T as Trait<'a, 'b>>::Assoc: 'c` constraint which is
        // unsatisfiable in the root
        } expect {}
    }
}

fn main() {}
