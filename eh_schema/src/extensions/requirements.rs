use auto_ops::*;

use crate::schema::Requirement;

impl std::ops::Not for Requirement {
    type Output = Requirement;

    fn not(self) -> Self::Output {
        match self {
            Self::None(none) => Self::any().with_requirements(none.requirements).wrap(),
            Self::Any(any) => Self::none().with_requirements(any.requirements).wrap(),
            _ => Self::none().with_requirements([self]).wrap(),
        }
    }
}

impl std::ops::Not for &Requirement {
    type Output = Requirement;

    fn not(self) -> Self::Output {
        self.clone().not()
    }
}

macro_rules! op_binary {
    ($ty:ty { $($op:tt $op_assign:tt as $func:ident,)* }) => {
        $(
            impl_op!($op |a: $ty, b: $ty| -> $ty { $func(a, b) });
            impl_op!($op |a: $ty, b: &$ty| -> $ty { $func(a, b.clone()) });
            impl_op!($op |a: &$ty, b: $ty| -> $ty { $func(a.clone(), b) });
            impl_op!($op |a: &$ty, b: &$ty| -> $ty { $func(a.clone(), b.clone()) });

            impl_op!($op_assign |a: &mut $ty, b: $ty| { let a_data = std::mem::take(a); *a = $func(a_data, b) });
            impl_op!($op_assign |a: &mut $ty, b: &$ty| { let a_data = std::mem::take(a); *a = $func(a_data, b.clone()) });
        )*
    };
}

op_binary!(Requirement {
    & &= as bitand,
    | |= as bitor,
    ^ ^= as bitxor,
});

fn bitand(a: Requirement, b: Requirement) -> Requirement {
    match (a, b) {
        (Requirement::All(mut a), Requirement::All(b)) => {
            a.requirements.extend(b.requirements);
            Requirement::all().with_requirements(a.requirements)
        }
        (Requirement::All(mut a), b) => {
            a.requirements.push(b);
            Requirement::all().with_requirements(a.requirements)
        }
        (a, Requirement::All(mut b)) => {
            b.requirements.push(a);
            Requirement::all().with_requirements(b.requirements)
        }

        (a, b) => Requirement::all().with_requirements([a, b]),
    }
    .wrap()
}

fn bitor(a: Requirement, b: Requirement) -> Requirement {
    match (a, b) {
        (Requirement::Any(mut a), Requirement::Any(b)) => {
            a.requirements.extend(b.requirements);
            Requirement::any().with_requirements(a.requirements)
        }
        (Requirement::Any(mut a), b) => {
            a.requirements.push(b);
            Requirement::any().with_requirements(a.requirements)
        }
        (a, Requirement::Any(mut b)) => {
            b.requirements.push(a);
            Requirement::any().with_requirements(b.requirements)
        }

        (a, b) => Requirement::any().with_requirements([a, b]),
    }
    .wrap()
}

fn bitxor(a: Requirement, b: Requirement) -> Requirement {
    (a.clone() | b.clone()) & !(a & b)
}

impl From<()> for Requirement {
    fn from(_: ()) -> Self {
        Self::empty().wrap()
    }
}
