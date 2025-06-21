#![warn(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

//! This module provides a rules engine for games, allowing for the definition and application of game rules to a game state.

use serde::{de::DeserializeOwned, Serialize};
use std::fmt::Debug;

#[derive(Debug, PartialEq)]
pub enum RuleResult {
    Complete,
    WaitingForEvent,
    GameOver,
}

pub trait GameState: Default {}

pub trait GameEvent: Debug + DeserializeOwned + Serialize {}

pub trait Rule<GameStateT: GameState, GameEventT: GameEvent>: Send + Sync {
    fn apply(&self, state: &mut GameStateT) -> RuleResult;
    fn validate(&self, state: &GameStateT, event: &GameEventT) -> bool;
    fn consume(&self, state: &mut GameStateT, event: &GameEventT) -> RuleResult;
}

pub type RuleList<S, E> = Vec<Box<dyn Rule<S, E>>>;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EngineStatus {
    Ready,
    WaitingForEvent,
    GameOver,
}

pub struct RulesEngine<GameStateT: GameState, GameEventT: GameEvent> {
    game_state: GameStateT,
    current_rule_chain: RuleList<GameStateT, GameEventT>,
    current_rule_index: usize,
    engine_status: EngineStatus,
}

impl<GameStateT: GameState, GameEventT: GameEvent> RulesEngine<GameStateT, GameEventT> {
    pub fn new(rule_chain: Vec<Box<dyn Rule<GameStateT, GameEventT>>>) -> Self {
        Self {
            game_state: GameStateT::default(),
            current_rule_chain: rule_chain,
            current_rule_index: 0,
            engine_status: EngineStatus::Ready,
        }
    }

    pub fn new_with_state(
        rule_chain: Vec<Box<dyn Rule<GameStateT, GameEventT>>>,
        initial_state: GameStateT,
    ) -> Self {
        Self {
            game_state: initial_state,
            current_rule_chain: rule_chain,
            current_rule_index: 0,
            engine_status: EngineStatus::Ready,
        }
    }

    pub fn game_state(&self) -> &GameStateT {
        &self.game_state
    }

    pub fn current_rule_index(&self) -> usize {
        self.current_rule_index
    }

    pub fn is_waiting_for_event(&self) -> bool {
        self.engine_status == EngineStatus::WaitingForEvent
    }

    pub fn engine_status(&self) -> EngineStatus {
        self.engine_status
    }

    // Process next rule
    pub fn process_next_rule(&mut self) {
        if self.current_rule_index >= self.current_rule_chain.len() {
            println!("[Engine] Turn complete!");
            return;
        }

        let rule = &self.current_rule_chain[self.current_rule_index];
        let result = rule.apply(&mut self.game_state);
        self.consume_rule_result(result);
    }

    pub fn validate(&self, event: &GameEventT) -> bool {
        if self.current_rule_index >= self.current_rule_chain.len() {
            return false;
        }

        let current_rule = &self.current_rule_chain[self.current_rule_index];
        current_rule.validate(&self.game_state, event)
    }

    pub fn consume(&mut self, event: &GameEventT) {
        if !self.is_waiting_for_event() {
            println!("[Engine] Ingorning unexpected event: '{:?}'", event);
            return;
        }

        assert!(self.current_rule_index < self.current_rule_chain.len());

        println!("[Engine] Received event '{:?}', resuming...", event);
        debug_assert!(self.validate(event));
        let rule = &self.current_rule_chain[self.current_rule_index];
        let result = rule.consume(&mut self.game_state, event);
        self.consume_rule_result(result);
    }

    fn consume_rule_result(&mut self, result: RuleResult) {
        match result {
            RuleResult::Complete => {
                self.engine_status = EngineStatus::Ready;
                self.current_rule_index += 1;
                self.process_next_rule();
            }
            RuleResult::WaitingForEvent => {
                self.engine_status = EngineStatus::WaitingForEvent;
            }
            RuleResult::GameOver => {
                println!("[Engine] Game over");
                self.engine_status = EngineStatus::GameOver;
            }
        };
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    #[derive(Default)]
    struct TestGameState {
        sum: u32,
        waiting_for_event: Option<TestWaitingFor>,
    }

    impl GameState for TestGameState {}

    #[derive(Debug, Serialize, Deserialize)]
    enum TestGameEvent {
        AddNumber { number: u32 },
        PlaceholderEvent, // Placeholder for other events, prevents irrefultable_let_patterns warnings
    }

    impl GameEvent for TestGameEvent {}

    #[derive(Debug, PartialEq)]
    enum TestWaitingFor {
        AddNumber,
    }

    type TestRuleList = RuleList<TestGameState, TestGameEvent>;

    /// A rule that adds even numbers to the sum in the game state. It waits for a
    /// [TestGameEvent::AddNumber], validates that the number is event, and applies the event to the
    /// game state.
    #[derive(Debug)]
    struct AddEvenNumbersRule;

    impl Rule<TestGameState, TestGameEvent> for AddEvenNumbersRule {
        fn apply(&self, state: &mut TestGameState) -> RuleResult {
            assert!(state.waiting_for_event.is_none());
            assert_eq!(state.sum, 0);
            state.waiting_for_event = Some(TestWaitingFor::AddNumber);
            RuleResult::WaitingForEvent
        }

        fn validate(&self, _state: &TestGameState, event: &TestGameEvent) -> bool {
            let TestGameEvent::AddNumber { number } = event else {
                return false;
            };

            *number % 2 == 0
        }

        fn consume(&self, state: &mut TestGameState, event: &TestGameEvent) -> RuleResult {
            let TestGameEvent::AddNumber { number } = event else {
                panic!("{:?} received unexpected event: {:?}", self, event);
            };

            state.sum += number;
            state.waiting_for_event = None;
            RuleResult::Complete
        }
    }

    #[test]
    fn test_rules_engine() {
        let rule_chain: TestRuleList = vec![Box::new(AddEvenNumbersRule)];
        let mut engine = RulesEngine::new(rule_chain);

        assert_eq!(engine.game_state.sum, 0);
        assert!(engine.game_state.waiting_for_event.is_none());

        assert_eq!(engine.engine_status(), EngineStatus::Ready);
        assert_eq!(engine.is_waiting_for_event(), false);
        assert_eq!(engine.current_rule_index(), 0);

        engine.process_next_rule();

        let verify_rule_0_is_waiting_for_event = || {
            assert_eq!(engine.current_rule_index(), 0);
            assert_eq!(engine.engine_status(), EngineStatus::WaitingForEvent);
            assert_eq!(engine.is_waiting_for_event(), true);
        };
        verify_rule_0_is_waiting_for_event();

        // Expect that an invalid event is rejected by the engine, and does not modify the game state
        let invalid_event = TestGameEvent::AddNumber { number: 1 };
        assert_eq!(engine.validate(&invalid_event), false);
        assert_eq!(engine.game_state.sum, 0);

        verify_rule_0_is_waiting_for_event();

        // Expect that a valid event is accepted by the engine, but does not modify the game state
        let valid_event = TestGameEvent::AddNumber { number: 2 };
        assert_eq!(engine.validate(&valid_event), true);
        assert_eq!(engine.game_state.sum, 0);

        verify_rule_0_is_waiting_for_event();

        // Expect that consuming a valid event updates the game state and progresses to the next rule
        engine.consume(&valid_event);
        assert_eq!(engine.game_state.sum, 2);
        assert!(engine.game_state.waiting_for_event.is_none());

        // Verify that the engine has moved to the next rule
        assert_eq!(engine.current_rule_index(), 1);
        assert_eq!(engine.engine_status(), EngineStatus::Ready);
        assert_eq!(engine.is_waiting_for_event(), false);
    }
}
