//@ compile-flags: -Znext-solver
// check-pass

pub trait QueryDb<'d>: Sized {
    type DynDb: HasQueryGroup<Self::Group>;

    type Group: QueryGroup;
}

pub trait QueryGroup: Sized {
    type DynDb: HasQueryGroup<Self>;
}

pub trait HasQueryGroup<G>
where
    G: QueryGroup,
{
}

pub trait EqualDynDb<'d, IQ: QueryDb<'d>>: QueryDb<'d> {}

impl<'d, IQ, Q> EqualDynDb<'d, IQ> for Q
where
    Q: QueryDb<'d, DynDb = IQ::DynDb, Group = IQ::Group>,
    Q::DynDb: HasQueryGroup<IQ::Group>,
    IQ: QueryDb<'d>,
{
}

fn main() {}
