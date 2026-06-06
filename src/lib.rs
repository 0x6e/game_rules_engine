#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

//! This crate provides a rules engine for games, allowing for the definition and application of
//! game rules to a game state.

use serde::{Serialize, de::DeserializeOwned};
use std::fmt::Debug;

/// An identifier for a rule, which can be used for debugging, logging, and other purposes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RuleId(pub &'static str);

/// A trait representing the identity of a rule, which provides a static and instance method for
/// retrieving the rule's identifier.
pub trait RuleIdentity {
    /// The static [RuleId] for this rule.
    fn static_id() -> RuleId
    where
        Self: Sized;

    /// The [RuleId] for this rule.
    fn id(&self) -> RuleId;
}

/// A macro to implement the [RuleIdentity] trait for a rule.
///
/// The macro takes the type of the rule and an optional string literal to use as the rule id. If
/// the string literal is not provided, it will default to the name of the type as the rule id.
#[macro_export]
macro_rules! impl_rule_id {
    ($t:ty, $id:expr) => {
        impl RuleIdentity for $t {
            fn id(&self) -> RuleId {
                Self::static_id()
            }

            fn static_id() -> RuleId
            where
                Self: Sized,
            {
                RuleId($id)
            }
        }
    };
    ($t:ty) => {
        impl_rule_id!($t, stringify!($t));
    };
}

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
pub trait Rule<GameStateT: GameState, GameEventT: GameEvent>: RuleIdentity + Send + Sync {
    /// Applies the initial state of the rule to the game state, modifying it as necessary. Returns
    /// a [RuleResult] indicating the outcome.
    fn apply(&mut self, state: &mut GameStateT) -> RuleResult;

    /// Validates whether a given game event is applicable to the current game state and rule.
    fn validate(&self, state: &GameStateT, event: &GameEventT) -> bool;

    /// Consumes a game event, modifying the game state and returning a [RuleResult] indicating the
    /// outcome.
    fn consume(&mut self, state: &mut GameStateT, event: &GameEventT) -> RuleResult;

    /// Returns an optional mutable reference to a slice of child rules, if this rule has any. This
    /// is used for composite rules that contain a chain of sub-rules. If the rule does not have any
    /// child rules, it should return `None`.
    fn children(&self) -> Option<&RuleSlice<GameStateT, GameEventT>> {
        None
    }

    /// Like [Rule::children], but returns a mutable reference to the child rules.
    fn children_mut(&mut self) -> Option<&mut RuleSlice<GameStateT, GameEventT>> {
        None
    }
}

/// A type alias for a list of rules, where each rule is a boxed trait object that implements the
/// [Rule] trait.
pub type RuleList<S, E> = Vec<Box<dyn Rule<S, E>>>;

/// A type alias for a slice into a [RuleList].
pub type RuleSlice<S, E> = [Box<dyn Rule<S, E>>];

/// A composite rule that contains a chain of sub-rules. This allows for the creation of complex
/// rules that consist of multiple steps, where each step is represented by a separate rule. The
/// composite rule itself does not have any behavior, it simply serves as a container for the chain
/// of sub-rules.
pub struct CompositeRule<GameStateT: GameState, GameEventT: GameEvent> {
    id: RuleId,
    rules: RuleList<GameStateT, GameEventT>,
}

impl<GameStateT: GameState, GameEventT: GameEvent> CompositeRule<GameStateT, GameEventT> {
    /// Creates a new [CompositeRule] with the given list of rules.
    pub fn new(id: RuleId, rule_chain: RuleList<GameStateT, GameEventT>) -> Self {
        Self {
            id,
            rules: rule_chain,
        }
    }
}

impl<GameStateT: GameState, GameEventT: GameEvent> RuleIdentity
    for CompositeRule<GameStateT, GameEventT>
{
    fn static_id() -> RuleId
    where
        Self: Sized,
    {
        RuleId("CompositeRule")
    }

    fn id(&self) -> RuleId {
        self.id
    }
}

impl<GameStateT: GameState, GameEventT: GameEvent> Rule<GameStateT, GameEventT>
    for CompositeRule<GameStateT, GameEventT>
{
    /// The [CompositeRule] itself does not have any behavior, so applying it simply returns
    /// [RuleResult::Complete].
    fn apply(&mut self, _: &mut GameStateT) -> RuleResult {
        RuleResult::Complete
    }

    /// The [CompositeRule] itself does not have any behavior, so it does not validate any events.
    /// This always returns false, and the individual sub-rules are responsible for validating
    /// events as necessary.
    fn validate(&self, _: &GameStateT, _: &GameEventT) -> bool {
        false
    }

    /// The [CompositeRule] itself does not have any behavior, so consuming an event simply returns
    /// [RuleResult::Complete]. The individual sub-rules are responsible for consuming events and
    /// modifying the game state as necessary.
    fn consume(&mut self, _: &mut GameStateT, _: &GameEventT) -> RuleResult {
        RuleResult::Complete
    }

    /// Returns the child rules which make up the composite rule.
    fn children(&self) -> Option<&RuleSlice<GameStateT, GameEventT>> {
        Some(&self.rules)
    }

    /// Like [Self::children], but returns a mutable reference to the child rules.
    fn children_mut(&mut self) -> Option<&mut RuleSlice<GameStateT, GameEventT>> {
        Some(&mut self.rules)
    }
}

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
    rule_stack: Vec<usize>,
    rule_stack_ids: Vec<RuleId>,
    started: bool,
    engine_status: EngineStatus,
}

impl<GameStateT: GameState, GameEventT: GameEvent> Default for RulesEngine<GameStateT, GameEventT> {
    fn default() -> Self {
        Self {
            game_state: Default::default(),
            current_rule_chain: Default::default(),
            rule_stack: Default::default(),
            rule_stack_ids: Default::default(),
            started: false,
            engine_status: EngineStatus::Ready,
        }
    }
}

impl<GameStateT: GameState, GameEventT: GameEvent> RulesEngine<GameStateT, GameEventT> {
    /// Creates a new [RulesEngine] with default game state and a specified rule chain.
    pub fn new(rule_chain: Vec<Box<dyn Rule<GameStateT, GameEventT>>>) -> Self {
        Self {
            current_rule_chain: rule_chain,
            ..Default::default()
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
            ..Default::default()
        }
    }

    /// Returns a reference to the current game state.
    pub fn game_state(&self) -> &GameStateT {
        &self.game_state
    }

    /// Returns a reference to the current rule stack, which represents the path through the rule
    /// chain to the current rule being processed.
    pub fn rule_stack(&self) -> &[usize] {
        &self.rule_stack
    }

    /// Returns a reference to a vector of [RuleId]s representing the path through the rule chain to
    /// the current rule being processed.
    pub fn current_rule_id(&self) -> &Vec<RuleId> {
        &self.rule_stack_ids
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
        if !self.started {
            println!("[Engine] Starting rules engine...");
            debug_assert!(self.rule_stack.is_empty());
            self.descend_into_rulechain();
            self.started = true;
        }

        if self.is_waiting_for_event() {
            println!("[Engine] Waiting for event, skipping rule application");
            return;
        }

        let Some(rule) =
            Self::current_rule_mut(self.current_rule_chain.as_mut_slice(), &self.rule_stack)
        else {
            println!("[Engine] Turn complete!");
            return;
        };

        match rule.children() {
            Some(children) => {
                debug_assert!(!children.is_empty());
                self.descend_into_rulechain();
                self.process_rules();
            }
            None => {
                let result = rule.apply(&mut self.game_state);
                self.consume_rule_result(result);
            }
        }
    }

    /// Returns true if the event is valid for the current rule, false otherwise.
    pub fn validate(&self, event: &GameEventT) -> bool {
        match Self::current_rule(self.current_rule_chain.as_slice(), &self.rule_stack) {
            Some(current_rule) => current_rule.validate(&self.game_state, event),
            None => {
                println!("[Engine] No current rule to validate against");
                false
            }
        }
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

        println!("[Engine] Received event '{:?}', resuming...", event);
        debug_assert!(self.validate(event));

        match Self::current_rule_mut(self.current_rule_chain.as_mut_slice(), &self.rule_stack) {
            Some(current_rule) => {
                let result = current_rule.consume(&mut self.game_state, event);
                self.consume_rule_result(result);
            }
            None => {
                println!("[Engine] No current rule to consume");
            }
        }
    }

    /// Consumes the result of applying a rule or consuming an event, updating the engine status and
    /// advancing to the next rule as necessary.
    fn consume_rule_result(&mut self, result: RuleResult) {
        match result {
            RuleResult::Complete => {
                self.engine_status = EngineStatus::Ready;
                self.advance();
                self.process_rules();
            }
            RuleResult::WaitingForEvent => {
                self.engine_status = EngineStatus::WaitingForEvent;
            }
            RuleResult::GameOver => {
                println!("[Engine] Game over");
                self.engine_status = EngineStatus::GameOver;
            }
        }
    }

    /// Returns a reference to the current rule being processed, or `None` if there is no current
    /// rule.
    fn current_rule<'a>(
        rule_chain: &'a RuleSlice<GameStateT, GameEventT>,
        rule_stack: &'a [usize],
    ) -> Option<&'a dyn Rule<GameStateT, GameEventT>> {
        let mut rules = rule_chain;

        for &idx in &rule_stack[..rule_stack.len().saturating_sub(1)] {
            let rule = &rules[idx];
            rules = rule.children()?;
        }

        let index = *rule_stack.last()?;
        let current_rule = rules.get(index)?;
        Some(current_rule.as_ref())
    }

    /// Returns a mutable reference to the current rule being processed, or `None` if there is no
    /// current rule.
    fn current_rule_mut<'a>(
        rule_chain: &'a mut RuleSlice<GameStateT, GameEventT>,
        rule_stack: &'a [usize],
    ) -> Option<&'a mut Box<dyn Rule<GameStateT, GameEventT>>> {
        let mut rules = rule_chain;

        for &idx in &rule_stack[..rule_stack.len().saturating_sub(1)] {
            let rule = &mut rules[idx];
            rules = rule.children_mut()?;
        }

        rules.get_mut(*rule_stack.last()?)
    }

    /// Attempt to step down a level in the rule chain.
    fn descend_into_rulechain(&mut self) {
        self.rule_stack.push(0);
        match Self::current_rule(&self.current_rule_chain, &self.rule_stack) {
            Some(new_rule) => {
                self.rule_stack_ids.push(new_rule.id());
            }
            None => {
                self.rule_stack.pop();
                println!("[Engine] No rules to apply");
            }
        }
    }

    /// Advances the rule stack to the next rule. If the current rule has siblings, it will move to
    /// the next sibling. If the current rule is the last sibling, it will pop up to the parent rule
    /// and advance it, recursively advancing up the stack as necessary. If the stack is at the top
    /// level and there are no more rules to advance to, it will simply return without modifying the
    /// stack.
    fn advance(&mut self) {
        let siblings = Self::current_sibling_count(&self.current_rule_chain, &self.rule_stack);

        let Some(last) = self.rule_stack.last_mut() else {
            return;
        };

        *last += 1;
        if *last < siblings {
            let next_id = Self::current_rule(&self.current_rule_chain, &self.rule_stack)
                .expect("Expected next rule to exist")
                .id();
            if let Some(last) = self.rule_stack_ids.last_mut() {
                *last = next_id;
            }
            return;
        }

        self.rule_stack.pop();
        self.rule_stack_ids.pop();
        self.advance();
    }

    /// Returns the number of siblings of the current rule, which is the number of rules in the
    /// current rule chain at the current level of the stack.
    fn current_sibling_count<'a>(
        rule_chain: &'a RuleSlice<GameStateT, GameEventT>,
        rule_stack: &'a [usize],
    ) -> usize {
        let mut rules = rule_chain;

        for &idx in &rule_stack[..rule_stack.len().saturating_sub(1)] {
            let rule = &rules[idx];
            match rule.children() {
                Some(r) => rules = r,
                None => return 0,
            };
        }

        rules.len()
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
    impl_rule_id!(AddEvenNumbersRule);

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
    impl_rule_id!(SubtractTenRule, "SubtractTenRuleId");

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

    /// A [CompositeRule] that combines [AddEvenNumbersRule] and [SubtractTenRule]. Demonstrates how
    /// to provide a specific combination of rules as a composite rule.
    struct AddEvenNumbersThenSubtractTenRule;
    impl_rule_id!(AddEvenNumbersThenSubtractTenRule);

    impl AddEvenNumbersThenSubtractTenRule {
        pub fn new() -> CompositeRule<TestGameState, TestGameEvent> {
            CompositeRule::new(
                Self::static_id(),
                vec![Box::new(AddEvenNumbersRule), Box::new(SubtractTenRule)],
            )
        }
    }

    #[test]
    fn verify_rule_id_macro() {
        assert_eq!(
            AddEvenNumbersRule::static_id(),
            RuleId("AddEvenNumbersRule")
        );

        let rule = AddEvenNumbersRule;
        assert_eq!(rule.id(), AddEvenNumbersRule::static_id());

        let rule: &dyn Rule<TestGameState, TestGameEvent> = &rule;
        assert_eq!(rule.id(), AddEvenNumbersRule::static_id());
    }

    #[test]
    fn verify_rule_id_macro_with_custom_id() {
        assert_eq!(SubtractTenRule::static_id(), RuleId("SubtractTenRuleId"));

        let rule = SubtractTenRule;
        assert_eq!(rule.id(), SubtractTenRule::static_id());

        let rule: &dyn Rule<TestGameState, TestGameEvent> = &rule;
        assert_eq!(rule.id(), SubtractTenRule::static_id());
    }

    #[test]
    fn verify_composite_rule_id() {
        assert_eq!(
            CompositeRule::<TestGameState, TestGameEvent>::static_id(),
            RuleId("CompositeRule")
        );

        assert_eq!(
            AddEvenNumbersThenSubtractTenRule::static_id(),
            RuleId("AddEvenNumbersThenSubtractTenRule")
        );

        let composite_rule = AddEvenNumbersThenSubtractTenRule::new();
        assert_eq!(
            composite_rule.id(),
            AddEvenNumbersThenSubtractTenRule::static_id()
        );

        let rule: &dyn Rule<TestGameState, TestGameEvent> = &composite_rule;
        assert_eq!(rule.id(), AddEvenNumbersThenSubtractTenRule::static_id());
    }

    #[test]
    fn verify_rules_engine_initial_state() {
        let rule_chain: TestRuleList = vec![Box::new(AddEvenNumbersRule)];
        let engine = RulesEngine::new(rule_chain);

        assert!(engine.rule_stack().is_empty());
        assert!(engine.current_rule_id().is_empty());
        assert_eq!(engine.started, false);
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
        assert_eq!(engine.rule_stack(), [0]);
        assert_eq!(*engine.current_rule_id(), [AddEvenNumbersRule::static_id()]);
        assert_eq!(engine.started, true);
        assert_eq!(engine.engine_status(), EngineStatus::WaitingForEvent);
        assert_eq!(engine.is_waiting_for_event(), true);

        // Verify that calling process_rules again does not call apply again
        engine.process_rules();

        // We should still be waiting for an event, and the rule should not have been applied again
        assert_eq!(engine.rule_stack(), [0]);
        assert_eq!(*engine.current_rule_id(), [AddEvenNumbersRule::static_id()]);
        assert_eq!(engine.started, true);
        assert_eq!(engine.engine_status(), EngineStatus::WaitingForEvent);
        assert_eq!(engine.is_waiting_for_event(), true);
    }

    #[test]
    fn apply_may_complete_a_rule() {
        let rule_chain: TestRuleList = vec![Box::new(SubtractTenRule)];
        apply_may_complete_a_rule_impl(rule_chain);
    }

    #[test]
    fn apply_may_complete_a_rule_composite() {
        let rule_chain: TestRuleList = vec![Box::new(CompositeRule::new(
            RuleId("TestCompositeRule"),
            vec![Box::new(SubtractTenRule)],
        ))];
        apply_may_complete_a_rule_impl(rule_chain);
    }

    fn apply_may_complete_a_rule_impl(rule_chain: TestRuleList) {
        let mut engine = RulesEngine::new(rule_chain);

        // Begin processing rules
        engine.process_rules();

        // Verify that the rule was applied and completed
        assert!(engine.rule_stack().is_empty());
        assert_eq!(engine.started, true);
        assert_eq!(engine.engine_status(), EngineStatus::Ready);
        assert_eq!(engine.is_waiting_for_event(), false);
        assert_eq!(engine.game_state.sum, -10); // Default state is 0, so it should be -10 now
    }

    #[test]
    fn test_rules_engine() {
        let rule_chain: TestRuleList =
            vec![Box::new(AddEvenNumbersRule), Box::new(SubtractTenRule)];
        test_rules_engine_impl(rule_chain, vec![0], vec![AddEvenNumbersRule::static_id()]);
    }

    #[test]
    fn test_rules_engine_composite() {
        let inner_rule_chain: TestRuleList =
            vec![Box::new(AddEvenNumbersRule), Box::new(SubtractTenRule)];
        let composite_rule_id = RuleId("TestCompositeRule");
        let rule_chain: TestRuleList = vec![Box::new(CompositeRule::new(
            composite_rule_id,
            inner_rule_chain,
        ))];
        test_rules_engine_impl(
            rule_chain,
            vec![0, 0],
            vec![composite_rule_id, AddEvenNumbersRule::static_id()],
        );
    }

    #[test]
    fn test_rules_engine_nested_composite() {
        let inner_rule_chain: TestRuleList =
            vec![Box::new(AddEvenNumbersRule), Box::new(SubtractTenRule)];
        let inner_composite_rule_id = RuleId("TestInnerCompositeRule");
        let inner_rule_chain: TestRuleList = vec![Box::new(CompositeRule::new(
            inner_composite_rule_id,
            inner_rule_chain,
        ))];
        let outer_composite_rule_id = RuleId("TestOuterCompositeRule");
        let rule_chain: TestRuleList = vec![Box::new(CompositeRule::new(
            outer_composite_rule_id,
            inner_rule_chain,
        ))];
        test_rules_engine_impl(
            rule_chain,
            vec![0, 0, 0],
            vec![
                outer_composite_rule_id,
                inner_composite_rule_id,
                AddEvenNumbersRule::static_id(),
            ],
        );
    }

    #[test]
    fn test_rules_engine_named_composite() {
        let rule_chain: TestRuleList = vec![Box::new(AddEvenNumbersThenSubtractTenRule::new())];
        test_rules_engine_impl(
            rule_chain,
            vec![0, 0],
            vec![
                AddEvenNumbersThenSubtractTenRule::static_id(),
                AddEvenNumbersRule::static_id(),
            ],
        );
    }

    fn test_rules_engine_impl(
        rule_chain: TestRuleList,
        expected_rule_stack: Vec<usize>,
        exected_rule_id: Vec<RuleId>,
    ) {
        let mut engine = RulesEngine::new(rule_chain);

        engine.process_rules();

        let verify_rule_0_is_waiting_for_event = || {
            assert_eq!(engine.rule_stack(), expected_rule_stack);
            assert_eq!(*engine.current_rule_id(), exected_rule_id);
            assert_eq!(engine.started, true);
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
        assert!(engine.game_state.waiting_for_event.is_none());

        // Expect that the SubtractTenRule has been applied
        assert_eq!(engine.game_state.sum, -8);

        // Verify that the engine has moved to the next rule
        assert!(engine.rule_stack().is_empty());
        assert!(engine.current_rule_id().is_empty());
        assert_eq!(engine.engine_status(), EngineStatus::Ready);
        assert_eq!(engine.is_waiting_for_event(), false);
    }
}
