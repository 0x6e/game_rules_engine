#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

//! This module provides a rules engine for games, allowing for the definition and application of
//! game rules to a game state.

use serde::{de::DeserializeOwned, Serialize};
use std::fmt::Debug;

/// The possible results of a [Rule] consuming a game event.
#[derive(Debug, PartialEq)]
pub enum RuleResult {
    /// The rule has completed successfully, and the engine can proceed to the next rule.
    Complete,

    /// The rule is waiting for an event to be consumed before it can proceed.
    WaitingForEvent,

    /// The game is over, and no further rules can be processed.
    GameOver,
}

/// A trait representing the game state, which must implement `Default` to allow for initialization.
pub trait GameState: Default {}

/// A trait representing a game event, which must implement `Debug`, `DeserializeOwned`, and
/// `Serialize` for serialization and deserialization.
pub trait GameEvent: Debug + DeserializeOwned + Serialize {}

/// A trait representing a rule that can be applied to a game state. It consumes game events and
/// modifies the game state accordingly.
pub trait Rule<GameStateT: GameState, GameEventT: GameEvent>: Send + Sync {
    /// Applies the initial state of the rule to the game state, modifying it as necessary. Returns
    /// a [RuleResult] indicating the outcome.
    fn apply(&mut self, state: &mut GameStateT) -> RuleResult;

    /// Validates whether a given game event is applicable to the current game state and rule.
    fn validate(&self, state: &GameStateT, event: &GameEventT) -> bool;

    /// Consumes a game event, modifying the game state and returning a [RuleResult] indicating the
    /// outcome.
    fn consume(&mut self, state: &mut GameStateT, event: &GameEventT) -> RuleResult;
}

/// A type alias for a list of rules, where each rule is a boxed trait object that implements the
/// [Rule] trait.
pub type RuleList<S, E> = Vec<Box<dyn Rule<S, E>>>;

/// An enumeration representing the status of the rules engine.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EngineStatus {
    /// The engine is ready to process rules.
    Ready,

    /// The engine is waiting for an event to be consumed before proceeding with the next rule.
    WaitingForEvent,

    /// The engine has reached a game-over state, and no further rules can be processed.
    GameOver,
}

/// A rules engine that processes a chain of rules against a game state and handles game events.
pub struct RulesEngine<GameStateT: GameState, GameEventT: GameEvent> {
    game_state: GameStateT,
    current_rule_chain: RuleList<GameStateT, GameEventT>,
    current_rule_index: usize,
    engine_status: EngineStatus,
}

impl<GameStateT: GameState, GameEventT: GameEvent> RulesEngine<GameStateT, GameEventT> {
    /// Creates a new [RulesEngine] with default game state and a specified rule chain.
    pub fn new(rule_chain: Vec<Box<dyn Rule<GameStateT, GameEventT>>>) -> Self {
        Self {
            game_state: GameStateT::default(),
            current_rule_chain: rule_chain,
            current_rule_index: 0,
            engine_status: EngineStatus::Ready,
        }
    }

    /// Creates a new [RulesEngine] with an initial game state and a rule chain.
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

    /// Returns a reference to the current game state.
    pub fn game_state(&self) -> &GameStateT {
        &self.game_state
    }

    /// Returns the index of the current rule being processed.
    pub fn current_rule_index(&self) -> usize {
        self.current_rule_index
    }

    /// Returns true if the engine is waiting for an event to be consumed, false otherwise.
    pub fn is_waiting_for_event(&self) -> bool {
        self.engine_status == EngineStatus::WaitingForEvent
    }

    /// Returns the current status of the rules engine.
    pub fn engine_status(&self) -> EngineStatus {
        self.engine_status
    }

    /// Processes the rules in the rule chain, processing as many rules as possible until either all
    /// rules are processed or the engine is waiting for an event.
    pub fn process_rules(&mut self) {
        if self.current_rule_index >= self.current_rule_chain.len() {
            println!("[Engine] Turn complete!");
            return;
        }

        if self.is_waiting_for_event() {
            println!("[Engine] Waiting for event, skipping rule application");
            return;
        }

        let rule = &mut self.current_rule_chain[self.current_rule_index];
        let result = rule.apply(&mut self.game_state);
        self.consume_rule_result(result);
    }

    /// Returns true if the event is valid for the current rule, false otherwise.
    pub fn validate(&self, event: &GameEventT) -> bool {
        if self.current_rule_index >= self.current_rule_chain.len() {
            return false;
        }

        let current_rule = &self.current_rule_chain[self.current_rule_index];
        current_rule.validate(&self.game_state, event)
    }

    /// Consumes a game event, processing it through the current rule and updating the game state
    /// accordingly. If the event causes the current rule to complete, it will continue processing
    /// as many rules as possible until either all rules are processed or the engine is waiting for
    /// an event.
    pub fn consume(&mut self, event: &GameEventT) {
        if !self.is_waiting_for_event() {
            println!("[Engine] Ingorning unexpected event: '{:?}'", event);
            return;
        }

        assert!(self.current_rule_index < self.current_rule_chain.len());

        println!("[Engine] Received event '{:?}', resuming...", event);
        debug_assert!(self.validate(event));
        let rule = &mut self.current_rule_chain[self.current_rule_index];
        let result = rule.consume(&mut self.game_state, event);
        self.consume_rule_result(result);
    }

    fn consume_rule_result(&mut self, result: RuleResult) {
        match result {
            RuleResult::Complete => {
                self.engine_status = EngineStatus::Ready;
                self.current_rule_index += 1;
                self.process_rules();
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
        sum: i32,
        waiting_for_event: Option<TestWaitingFor>,
    }

    impl GameState for TestGameState {}

    #[derive(Debug, Serialize, Deserialize)]
    enum TestGameEvent {
        AddNumber { number: i32 },
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
        fn apply(&mut self, state: &mut TestGameState) -> RuleResult {
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

        fn consume(&mut self, state: &mut TestGameState, event: &TestGameEvent) -> RuleResult {
            let TestGameEvent::AddNumber { number } = event else {
                panic!("{:?} received unexpected event: {:?}", self, event);
            };

            state.sum += number;
            state.waiting_for_event = None;
            RuleResult::Complete
        }
    }

    /// A rule that subtracts 10 from the sum in the game state. It does not validate or consume, it
    /// expects to be applied and immediately complete.
    #[derive(Debug)]
    struct SubtractTenRule;

    impl Rule<TestGameState, TestGameEvent> for SubtractTenRule {
        fn apply(&mut self, state: &mut TestGameState) -> RuleResult {
            assert!(state.waiting_for_event.is_none());
            state.sum -= 10;
            RuleResult::Complete
        }

        fn validate(&self, _state: &TestGameState, _event: &TestGameEvent) -> bool {
            false
        }

        fn consume(&mut self, _state: &mut TestGameState, event: &TestGameEvent) -> RuleResult {
            panic!("{:?} received unexpected event: {:?}", self, event);
        }
    }

    #[test]
    fn verify_rules_engine_initial_state() {
        let rule_chain: TestRuleList = vec![Box::new(AddEvenNumbersRule)];
        let engine = RulesEngine::new(rule_chain);

        assert_eq!(engine.current_rule_index(), 0);
        assert_eq!(engine.engine_status(), EngineStatus::Ready);
        assert_eq!(engine.is_waiting_for_event(), false);
        assert_eq!(engine.game_state.sum, 0);
        assert!(engine.game_state.waiting_for_event.is_none());
    }

    #[test]
    fn verify_test_game_state_initial_state() {
        let state: TestGameState = TestGameState::default();
        assert_eq!(state.sum, 0);
        assert!(state.waiting_for_event.is_none());
    }

    #[test]
    fn process_rules_only_calls_apply_once() {
        let rule_chain: TestRuleList = vec![Box::new(AddEvenNumbersRule)];
        let mut engine = RulesEngine::new(rule_chain);

        // Begin processing rules
        engine.process_rules();

        // Verify that we are now waiting for an event
        assert_eq!(engine.current_rule_index(), 0);
        assert_eq!(engine.engine_status(), EngineStatus::WaitingForEvent);
        assert_eq!(engine.is_waiting_for_event(), true);

        // Verify that calling process_rules again does not call apply again
        engine.process_rules();

        // We should still be waiting for an event, and the rule should not have been applied again
        assert_eq!(engine.current_rule_index(), 0);
        assert_eq!(engine.engine_status(), EngineStatus::WaitingForEvent);
        assert_eq!(engine.is_waiting_for_event(), true);
    }

    #[test]
    fn apply_may_complete_a_rule() {
        let rule_chain: TestRuleList = vec![Box::new(SubtractTenRule)];
        let mut engine = RulesEngine::new(rule_chain);

        // Begin processing rules
        engine.process_rules();

        // Verify that the rule was applied and completed
        assert_eq!(engine.current_rule_index(), 1);
        assert_eq!(engine.engine_status(), EngineStatus::Ready);
        assert_eq!(engine.is_waiting_for_event(), false);
        assert_eq!(engine.game_state.sum, -10); // Default state is 0, so it should be -10 now
    }

    #[test]
    fn test_rules_engine() {
        let rule_chain: TestRuleList = vec![Box::new(AddEvenNumbersRule)];
        let mut engine = RulesEngine::new(rule_chain);

        engine.process_rules();

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
