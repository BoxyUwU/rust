//@ compile-flags: -Zassumptions-on-binders -Znext-solver=globally

// test that we resolve region variables in our alias outlives constraints when eagerly handling
// the alias outlives constraint. this currently errors but it shouldn't.

#![feature(test_binder_constraints)]

trait Trait<'a, 'b> {
    type Assoc;
}

core::test_binder_constraints! {
    impl<'c, T>
    where
        for<'a, 'b> T: Trait<'a, 'b>,
    {
        forall<'a> //~ ERROR: unable to satisfy constraints involving placeholders due to unknown implied bounds
        where
            for<'b> <T as Trait<'a, 'b>>::Assoc: 'c,
        {
            forall<'b> {
                exists<'a2, 'b2, 'c2> {
                    'a2: 'a, 'a: 'a2,
                    'b2: 'b, 'b: 'b2,
                    'c2: 'c, 'c: 'c2,
                    // destructured version of `predicate <T as Trait<'a2, 'b2>>::Assoc: 'c2`
                    or {
                        for<> <T as Trait<'a2, 'b2>>::Assoc: 'c2,
                        and { T: 'c2, 'a2: 'c2, 'b2: 'c2 }
                    }
                }
            } expect {
                // incorrectly rewritten to ambiguity currently
                //
                // handling this correctly requires us to eagerly handle `<T as Trait<'a1, 'b2>::Assoc: 'c2` by
                // resolving that to `<T as Trait<'a, 'b>>:Assoc: 'c` so that the only current-universe term is `b`.
                // Then we need to convert this into `for<'b> <T as Trait<'a, 'b>>::Assoc: 'c` to get a smaller-universe
                // constraint that we can propagate.
                //
                // Currently we do not resolve vars and also don't replace too-large-universe region variables with
                // bound variables so we also don't produce `for<'a, 'b, 'c> <T as Trait<'a, 'b>>::Assoc: 'c` like one
                // would expect without having the ability to resolve region variables.
                //
                // Though note that `for<'a, 'b, 'c> <T as Trait<'a, 'b>>:Assoc: 'c` would be incorrect to return as it
                // is too strict and would not be able to match against the assumption `for<'b> <T as Trait<'a, 'b>>::Assoc: 'c`. 
                ambiguity
            }
        }
    }
}

fn main() {}