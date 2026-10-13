//! Deterministic four-week studio rules. No storage, time source or random input.

use crate::{Result, fail};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const RULESET: &str = "studio-v1";
pub const CAPACITY_HOURS: u16 = 20;
pub const PRODUCT_HOURS: u16 = 24;
pub const STARTING_CASH_MINOR: i64 = 600_000;
pub const WEEKLY_OVERHEAD_MINOR: i64 = 80_000;
pub const WEEKS: u8 = 4;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    pub ruleset: String,
    pub week: u8,
    pub product_hours: u16,
    pub cash_minor: i64,
    pub orders: Vec<Order>,
    pub history: Vec<WeekResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Order {
    pub id: String,
    pub title: String,
    pub available_week: u8,
    pub deadline_week: u8,
    pub total_hours: u16,
    pub remaining_hours: u16,
    pub payment_minor: i64,
    pub status: String,
    pub paid_minor: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Allocation {
    pub target: String,
    pub hours: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrderOutcome {
    pub id: String,
    pub status: String,
    pub worked_hours: u16,
    pub remaining_hours: u16,
    pub payment_minor: i64,
    pub explanation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Goal {
    pub cash_met: bool,
    pub product_met: bool,
    pub commitments_met: bool,
    pub success: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeekResult {
    pub week: u8,
    pub allocations: Vec<Allocation>,
    pub used_hours: u16,
    pub unused_hours: u16,
    pub cash_before_minor: i64,
    pub cash_after_minor: i64,
    pub cash_delta_minor: i64,
    pub product_before_hours: u16,
    pub product_after_hours: u16,
    pub order_outcomes: Vec<OrderOutcome>,
    pub explanation: Vec<String>,
    pub goal: Goal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Choice {
    pub id: String,
    pub title: String,
    pub benefit: String,
    pub tradeoff: String,
    pub allocations: Vec<Allocation>,
    pub preview: WeekResult,
}

pub fn initial() -> State {
    let order =
        |id: &str, title: &str, available_week, deadline_week, total_hours, payment_minor| Order {
            id: id.into(),
            title: title.into(),
            available_week,
            deadline_week,
            total_hours,
            remaining_hours: total_hours,
            payment_minor,
            status: "offered".into(),
            paid_minor: 0,
        };
    State {
        ruleset: RULESET.into(),
        week: 1,
        product_hours: 0,
        cash_minor: STARTING_CASH_MINOR,
        orders: vec![
            order("atlas", "Atlas", 1, 2, 24, 280_000),
            order("birch", "Birch", 2, 3, 20, 240_000),
            order("cedar", "Cedar", 3, 4, 20, 360_000),
        ],
        history: Vec::new(),
    }
}

fn available(order: &Order, week: u8) -> bool {
    order.available_week <= week && matches!(order.status.as_str(), "offered" | "active" | "late")
}

fn transition(state: &State, allocations: &[Allocation]) -> Result<State> {
    if state.week > WEEKS {
        return fail("invalid_input", "The four-week studio has finished.");
    }
    let mut targets = BTreeSet::new();
    let mut used = 0_u16;
    for allocation in allocations {
        if allocation.hours == 0 || allocation.hours % 2 != 0 {
            return fail("invalid_input", "Allocate positive whole two-hour blocks.");
        }
        if !targets.insert(allocation.target.as_str()) {
            return fail("invalid_input", "Each allocation target must be unique.");
        }
        used = used
            .checked_add(allocation.hours)
            .filter(|hours| *hours <= CAPACITY_HOURS)
            .ok_or_else(|| {
                crate::DomainError::new("invalid_input", "Weekly allocations exceed 20 hours.")
            })?;
        let remaining = if allocation.target == "product" {
            PRODUCT_HOURS - state.product_hours
        } else {
            let Some(order) = state
                .orders
                .iter()
                .find(|order| order.id == allocation.target)
            else {
                return fail(
                    "invalid_input",
                    "Allocation target is not part of this studio.",
                );
            };
            if !available(order, state.week) {
                return fail(
                    "invalid_input",
                    "This order is unavailable or already closed.",
                );
            }
            order.remaining_hours
        };
        if allocation.hours > remaining {
            return fail(
                "invalid_input",
                "Allocated hours exceed the target's remaining work.",
            );
        }
    }

    let mut next = state.clone();
    let mut income = 0;
    let mut outcomes = Vec::new();
    for order in &mut next.orders {
        let hours = allocations
            .iter()
            .find(|allocation| allocation.target == order.id)
            .map_or(0, |allocation| allocation.hours);
        let mut payment = 0;
        let explanation;
        if order.status == "delivered" {
            explanation = "Previously delivered; no additional payment.".into();
        } else if order.status == "expired" {
            explanation = "Unaccepted offer expired; no commitment or payment.".into();
        } else if order.available_week > state.week {
            explanation = format!(
                "Available from week {}; deadline at the end of week {}.",
                order.available_week, order.deadline_week
            );
        } else {
            if hours > 0 {
                order.remaining_hours -= hours;
                if order.status == "offered" {
                    order.status = "active".into();
                }
            }
            if order.remaining_hours == 0 {
                let on_time = state.week <= order.deadline_week;
                payment = if on_time {
                    order.payment_minor
                } else {
                    order.payment_minor / 4 * 3
                };
                order.paid_minor = payment;
                order.status = "delivered".into();
                explanation = format!(
                    "Worked {hours} hours and delivered {}: paid {payment} minor units ({}%); deadline week {}.",
                    if on_time { "on time" } else { "late" },
                    if on_time { 100 } else { 75 },
                    order.deadline_week
                );
            } else if state.week >= order.deadline_week && order.status == "offered" {
                order.status = "expired".into();
                explanation = format!(
                    "Not accepted by the end of deadline week {}; offer expired with no payment.",
                    order.deadline_week
                );
            } else if state.week >= order.deadline_week {
                order.status = "late".into();
                explanation = format!(
                    "Worked {hours} hours; {} hours remain after deadline week {}. Completion in a later week pays 75%; no payment yet.",
                    order.remaining_hours, order.deadline_week
                );
            } else {
                explanation = format!(
                    "Worked {hours} hours; {} hours remain, deadline week {}; {}. No payment yet.",
                    order.remaining_hours,
                    order.deadline_week,
                    if order.status == "offered" {
                        "offer not accepted"
                    } else {
                        "delivery committed"
                    }
                );
            }
        }
        income += payment;
        outcomes.push(OrderOutcome {
            id: order.id.clone(),
            status: order.status.clone(),
            worked_hours: hours,
            remaining_hours: order.remaining_hours,
            payment_minor: payment,
            explanation,
        });
    }
    let product_work = allocations
        .iter()
        .find(|allocation| allocation.target == "product")
        .map_or(0, |allocation| allocation.hours);
    next.product_hours += product_work;
    next.cash_minor += income - WEEKLY_OVERHEAD_MINOR;
    let cash_met = next.cash_minor >= STARTING_CASH_MINOR;
    let product_met = next.product_hours >= PRODUCT_HOURS;
    let commitments_met = !next
        .orders
        .iter()
        .any(|order| matches!(order.status.as_str(), "active" | "late"));
    let result = WeekResult {
        week: state.week,
        allocations: allocations.to_vec(),
        used_hours: used,
        unused_hours: CAPACITY_HOURS - used,
        cash_before_minor: state.cash_minor,
        cash_after_minor: next.cash_minor,
        cash_delta_minor: income - WEEKLY_OVERHEAD_MINOR,
        product_before_hours: state.product_hours,
        product_after_hours: next.product_hours,
        order_outcomes: outcomes,
        explanation: vec![
            format!(
                "Capacity: {used} of {CAPACITY_HOURS} hours used; {} hours unused.",
                CAPACITY_HOURS - used
            ),
            format!(
                "Payments: {income} minor units; fixed overhead: {WEEKLY_OVERHEAD_MINOR}; cash: {} to {}.",
                state.cash_minor, next.cash_minor
            ),
            format!(
                "Product work: {} to {} of {PRODUCT_HOURS} hours.",
                state.product_hours, next.product_hours
            ),
        ],
        goal: Goal {
            cash_met,
            product_met,
            commitments_met,
            success: state.week == WEEKS && cash_met && product_met && commitments_met,
        },
    };
    next.history.push(result);
    next.week += 1;
    Ok(next)
}

pub fn validate_state(state: &State) -> Result<()> {
    if state.history.len() > usize::from(WEEKS) {
        return fail("invalid_input", "Studio history exceeds four weeks.");
    }
    let mut replay = initial();
    for result in &state.history {
        let next = transition(&replay, &result.allocations)?;
        if next.history.last() != Some(result) {
            return fail(
                "invalid_input",
                "Studio history does not match its deterministic allocations.",
            );
        }
        replay = next;
    }
    if replay != *state {
        return fail(
            "invalid_input",
            "Studio state does not match a replay from studio-v1's opening.",
        );
    }
    Ok(())
}

pub fn preview(state: &State, allocations: &[Allocation]) -> Result<WeekResult> {
    validate_state(state)?;
    let next = transition(state, allocations)?;
    Ok(next
        .history
        .last()
        .expect("a completed week has a result")
        .clone())
}

pub fn advance(state: &State, allocations: &[Allocation]) -> Result<State> {
    validate_state(state)?;
    transition(state, allocations)
}

fn add_product(state: &State, allocations: &mut Vec<Allocation>, left: &mut u16, limit: u16) {
    let hours = (PRODUCT_HOURS - state.product_hours).min(*left).min(limit);
    if hours > 0 {
        allocations.push(Allocation {
            target: "product".into(),
            hours,
        });
        *left -= hours;
    }
}

fn add_orders(
    state: &State,
    allocations: &mut Vec<Allocation>,
    left: &mut u16,
    new_orders: bool,
    committed_first: bool,
) {
    let mut orders = state
        .orders
        .iter()
        .filter(|order| available(order, state.week) && (new_orders || order.status != "offered"))
        .collect::<Vec<_>>();
    orders.sort_by_key(|order| {
        (
            committed_first && order.status == "offered",
            order.deadline_week,
            order.id.as_str(),
        )
    });
    for order in orders {
        let hours = order.remaining_hours.min(*left);
        if hours > 0 {
            allocations.push(Allocation {
                target: order.id.clone(),
                hours,
            });
            *left -= hours;
        }
    }
}

pub fn choices(state: &State) -> Vec<Choice> {
    if validate_state(state).is_err() || state.week > WEEKS {
        return Vec::new();
    }
    let definitions = [
        (
            "deliver",
            "Prioritize deliveries",
            "Work on committed orders first and pursue available payments.",
            "Less time for the reusable product; starting an offer commits its deadline.",
        ),
        (
            "product",
            "Protect product time",
            "Build the reusable product first and avoid new delivery commitments.",
            "Cash from offers may be lost; existing orders receive only remaining capacity.",
        ),
        (
            "balance",
            "Reserve eight product hours",
            "Keep product progress while working on the earliest available deadlines.",
            "Splitting capacity can leave accepted deliveries late, reducing payment to 75%.",
        ),
    ];
    definitions
        .into_iter()
        .map(|(id, title, benefit, tradeoff)| {
            let mut allocations = Vec::new();
            let mut left = CAPACITY_HOURS;
            match id {
                "deliver" => {
                    add_orders(state, &mut allocations, &mut left, true, true);
                    add_product(state, &mut allocations, &mut left, CAPACITY_HOURS);
                }
                "product" => {
                    add_product(state, &mut allocations, &mut left, CAPACITY_HOURS);
                    add_orders(state, &mut allocations, &mut left, false, true);
                }
                _ => {
                    add_product(state, &mut allocations, &mut left, 8);
                    add_orders(state, &mut allocations, &mut left, true, false);
                }
            }
            let next =
                transition(state, &allocations).expect("built-in allocations satisfy studio-v1");
            Choice {
                id: id.into(),
                title: title.into(),
                benefit: benefit.into(),
                tradeoff: tradeoff.into(),
                allocations,
                preview: next
                    .history
                    .last()
                    .expect("a completed week has a result")
                    .clone(),
            }
        })
        .collect()
}
