//@ compile-flags: -Zassumptions-on-binders -Znext-solver=globally

// test that a `<T as AliasHaver>::Assoc: '!a_u1` constraint is considered to be satisfied
// if there's a `T::Assoc: 'static` assumption in the root universe and if not that it is
// an error :)

#![feature(test_binder_constraints, generic_const_items)]

trait AliasHaver {
    type Assoc;
}

core::test_binder_constraints! {
    impl<'a, T: AliasHaver>
    where
        <T as AliasHaver>::Assoc: 'static,
    {
        forall<'b> {
            where <T as AliasHaver>::Assoc: 'b
        } expect {
            or {
                for<'b> <T as AliasHaver>::Assoc: 'b,
                for<> <T as AliasHaver>::Assoc: 'static,
                T: 'static,
            }
        }
    }
}

core::test_binder_constraints! {
    impl<'a, T: AliasHaver>
    where
        <T as AliasHaver>::Assoc: 'a,
    {
        forall<'b> { //~ ERROR: higher-ranked lifetime bound could not be satisfied
            where <T as AliasHaver>::Assoc: 'b
        } expect {
            or {
                for<'b> <T as AliasHaver>::Assoc: 'b,
                for<> <T as AliasHaver>::Assoc: 'static,
                T: 'static,
            }
        }
    }
}

trait Trait<'a> {}
impl<'a, T: 'a> Trait<'a> for T {}
struct ReqTrait<T: for<'a> Trait<'a>>(T);

fn borrowck_env_pass<'a, T: AliasHaver>()
where
    <T as AliasHaver>::Assoc: 'static,
{
    let _: ReqTrait<T::Assoc>;
}

fn borrowck_env_fail<'a, T: AliasHaver>()
// FIXME: ^ this should raise an ERROR: unsatisfied lifetime constraint from -Zassumptions-on-binders
where
    <T as AliasHaver>::Assoc: 'a,
{
    let _: ReqTrait<T::Assoc>;
}

const REGIONCK_ENV_PASS<'a, T: AliasHaver>: ReqTrait<T::Assoc> = todo!()
where
    <T as AliasHaver>::Assoc: 'static;

const REGIONCK_ENV_FAIL<'a, T: AliasHaver>: ReqTrait<T::Assoc> = todo!()
//~^ ERROR: unable to satisfy constraints involving placeholders due to unknown implied bounds
//~| ERROR: unable to satisfy constraints involving placeholders due to unknown implied bounds
where
    <T as AliasHaver>::Assoc: 'a;

fn main() {}
