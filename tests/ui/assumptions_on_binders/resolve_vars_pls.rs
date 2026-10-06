//@ compile-flags: -Zassumptions-on-binders -Znext-solver=globally
//~^ ERROR: unable to satisfy region constraints in root

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
        forall<'a>
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
                // incorrectly rewritten to return higher ranked alias outlives with everything being bound variables.
                // this causes us to be unable to use the assumption `for<'b> <T as Trait<'a, 'b>>::Assoc: 'c` to prove
                // anything because we've "forgotten" that we had an `'a` or `'c` originally. 
                //
                // handling this correctly requires us to eagerly handle `<T as Trait<'a1, 'b2>::Assoc: 'c2` by
                // resolving that to `<T as Trait<'a, 'b>>:Assoc: 'c` so that the only current-universe term is `b`.
                // Then we need to convert this into `for<'b> <T as Trait<'a, 'b>>::Assoc: 'c` to get a lower-universe
                // constraint that we can propagate.
                or {
                    for<'a2, 'b2, 'c2> <T as Trait<'a2, 'b2>>::Assoc: 'c2,
                    for<'a2, 'b2> <T as Trait<'a2, 'b2>>::Assoc: 'static,
                }
            }
        }
    }
}

fn main() {}