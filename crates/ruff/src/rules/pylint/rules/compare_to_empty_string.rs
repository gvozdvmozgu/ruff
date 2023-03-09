use anyhow::bail;
use itertools::Itertools;
use rustpython_parser::ast::{Cmpop, Constant, Expr, ExprKind};

use ruff_diagnostics::{Diagnostic, Violation};
use ruff_macros::{derive_message_formats, violation};
use ruff_python_ast::helpers::{unparse_constant, unparse_expr};
use ruff_python_ast::types::Range;

use crate::checkers::ast::Checker;

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum EmptyStringCmpop {
    Is,
    IsNot,
    Eq,
    NotEq,
}

impl TryFrom<&Cmpop> for EmptyStringCmpop {
    type Error = anyhow::Error;

    fn try_from(value: &Cmpop) -> Result<Self, Self::Error> {
        match value {
            Cmpop::Is => Ok(Self::Is),
            Cmpop::IsNot => Ok(Self::IsNot),
            Cmpop::Eq => Ok(Self::Eq),
            Cmpop::NotEq => Ok(Self::NotEq),
            _ => bail!("{value:?} cannot be converted to EmptyStringCmpop"),
        }
    }
}

impl EmptyStringCmpop {
    pub fn into_unary(self) -> &'static str {
        match self {
            Self::Is | Self::Eq => "",
            Self::IsNot | Self::NotEq => "not ",
        }
    }
}

impl std::fmt::Display for EmptyStringCmpop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let repr = match self {
            Self::Is => "is",
            Self::IsNot => "is not",
            Self::Eq => "==",
            Self::NotEq => "!=",
        };
        write!(f, "{repr}")
    }
}

#[violation]
pub struct CompareToEmptyString {
    pub existing: String,
    pub replacement: String,
}

impl Violation for CompareToEmptyString {
    #[derive_message_formats]
    fn message(&self) -> String {
        format!(
            "`{}` can be simplified to `{}` as an empty string is falsey",
            self.existing, self.replacement,
        )
    }
}

pub fn compare_to_empty_string(
    checker: &mut Checker,
    left: &Expr,
    ops: &[Cmpop],
    comparators: &[Expr],
) {
    let mut first = true;
    for ((lhs, rhs), op) in std::iter::once(left)
        .chain(comparators.iter())
        .tuple_windows::<(&Expr<_>, &Expr<_>)>()
        .zip(ops)
    {
        if let Ok(op) = EmptyStringCmpop::try_from(op) {
            if std::mem::take(&mut first) {
                // Check the left-most expression.
                if let ExprKind::Constant { value, .. } = &lhs.node {
                    if let Constant::Str(s) = value {
                        if s.is_empty() {
                            let existing = format!(
                                "{} {} {}",
                                unparse_constant(value, checker.stylist),
                                op,
                                unparse_expr(rhs, checker.stylist)
                            );
                            let replacement = format!(
                                "{}{}",
                                op.into_unary(),
                                unparse_expr(rhs, checker.stylist)
                            );
                            checker.diagnostics.push(Diagnostic::new(
                                CompareToEmptyString {
                                    existing,
                                    replacement,
                                },
                                Range::from(lhs),
                            ));
                        }
                    }
                }
            }

            // Check all right-hand expressions.
            if let ExprKind::Constant { value, .. } = &rhs.node {
                if let Constant::Str(s) = value {
                    if s.is_empty() {
                        let existing = format!(
                            "{} {} {}",
                            unparse_expr(lhs, checker.stylist),
                            op,
                            unparse_constant(value, checker.stylist),
                        );
                        let replacement =
                            format!("{}{}", op.into_unary(), unparse_expr(lhs, checker.stylist));
                        checker.diagnostics.push(Diagnostic::new(
                            CompareToEmptyString {
                                existing,
                                replacement,
                            },
                            Range::from(rhs),
                        ));
                    }
                }
            }
        }
    }
}
