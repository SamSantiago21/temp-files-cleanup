use crate::{
    domain::{Condition, ConditionNode},
    errors::EngineError,
};
use std::time::{SystemTime, UNIX_EPOCH};
pub trait ConditionEvaluator {
    fn evaluate(&self, node: &ConditionNode) -> Result<bool, EngineError>;
}
#[derive(Default)]
pub struct SystemConditionEvaluator;
impl ConditionEvaluator for SystemConditionEvaluator {
    fn evaluate(&self, n: &ConditionNode) -> Result<bool, EngineError> {
        match n {
            ConditionNode::Empty => Ok(true),
            ConditionNode::And { children } => children
                .iter()
                .map(|x| self.evaluate(x))
                .try_fold(true, |a, r| Ok(a && r?)),
            ConditionNode::Or { children } => children
                .iter()
                .map(|x| self.evaluate(x))
                .try_fold(false, |a, r| Ok(a || r?)),
            ConditionNode::Not { child } => Ok(!self.evaluate(child)?),
            ConditionNode::Leaf { condition } => self.leaf(condition),
        }
    }
}
impl SystemConditionEvaluator {
    fn leaf(&self, c: &Condition) -> Result<bool, EngineError> {
        match c {
            Condition::TimeRange {
                start_hh_mm,
                end_hh_mm,
            } => {
                let now = current_minutes()?;
                let s = parse_minutes(start_hh_mm)?;
                let e = parse_minutes(end_hh_mm)?;
                Ok(if s <= e {
                    now >= s && now <= e
                } else {
                    now >= s || now <= e
                })
            }
            Condition::BatteryBelow { .. } => Ok(false),
        }
    }
}
fn parse_minutes(v: &str) -> Result<u16, EngineError> {
    let p: Vec<_> = v.split(':').collect();
    if p.len() != 2 {
        return Err(EngineError::InvalidTime(v.into()));
    }
    let h: u16 = p[0]
        .parse()
        .map_err(|_| EngineError::InvalidTime(v.into()))?;
    let m: u16 = p[1]
        .parse()
        .map_err(|_| EngineError::InvalidTime(v.into()))?;
    if h > 23 || m > 59 {
        return Err(EngineError::InvalidTime(v.into()));
    }
    Ok(h * 60 + m)
}
fn current_minutes() -> Result<u16, EngineError> {
    let s = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| EngineError::Action(e.to_string()))?
        .as_secs();
    Ok(((s / 60) % 1440) as u16)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tree_logic() {
        let e = SystemConditionEvaluator;
        let n = ConditionNode::And {
            children: vec![
                ConditionNode::Empty,
                ConditionNode::Not {
                    child: Box::new(ConditionNode::Or { children: vec![] }),
                },
            ],
        };
        assert!(e.evaluate(&n).unwrap())
    }
    #[test]
    fn invalid_time() {
        assert!(parse_minutes("25:00").is_err())
    }
}
