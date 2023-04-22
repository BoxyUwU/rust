use rustc_infer::traits::solve::{Certainty, Goal, QueryResult};
use rustc_middle::ty;

use super::search_graph::StackDepth;

pub struct GoalEvaluation<'tcx> {
    pub depth: StackDepth,

    /// Evaluated goal
    pub goal: Goal<'tcx, ty::Predicate<'tcx>>,
    /// Evaluated goal after canonicalization
    pub canonicalized_goal: Goal<'tcx, ty::Predicate<'tcx>>,

    pub nested_goals: Vec<GoalEvaluation<'tcx>>,
    pub candidates: Vec<Candidate<'tcx>>,

    /// Result from evaluating the canonical goal
    pub result: QueryResult<'tcx>,
    /// Certainty after instantiating response
    pub certainty: Certainty,
}

pub struct Candidate<'tcx> {
    pub name: String, // FIXME: represent this more typed for diagnostics
    pub result: QueryResult<'tcx>,
    pub nested_goals: Vec<GoalEvaluation<'tcx>>,
    pub candidates: Vec<Candidate<'tcx>>,
}

enum InspectBuilder {}
