use alloc::string::String;
use alloc::vec::Vec;
use core::{
    fmt::{self, Debug},
    marker::PhantomData,
};
use serde::{Deserialize, Serialize};

use crate::{
    bolts::tuples::{tuple_list, NamedTuple},
    corpus::Corpus,
    inputs::{HasBytesVec, Input},
    mutators::{MutationResult, Mutator, MutatorsTuple},
    state::{HasCorpus, HasMaxSize, HasMetadata, HasRand},
    utils::{AsSlice, Rand},
    Error,
};

pub use crate::mutators::mutations::*;
pub use crate::mutators::token_mutations::*;

#[derive(Serialize, Deserialize)]
pub struct MutationsMetadata {
    pub list: Vec<String>,
}

crate::impl_serdeany!(MutationsMetadata);

impl AsSlice<String> for MutationsMetadata {
    fn as_slice(&self) -> &[String] {
        self.list.as_slice()
    }
}

impl MutationsMetadata {
    pub fn new(list: Vec<String>) -> Self {
        Self { list }
    }
}

pub trait ComposedByMutations<I, MT, S>
where
    I: Input,
    MT: MutatorsTuple<I, S>,
{
    /// Get the mutations
    fn mutations(&self) -> &MT;

    // Get the mutations (mut)
    fn mutations_mut(&mut self) -> &mut MT;
}

pub trait ScheduledMutator<I, MT, S>: ComposedByMutations<I, MT, S> + Mutator<I, S>
where
    I: Input,
    MT: MutatorsTuple<I, S>,
{
    /// Compute the number of iterations used to apply stacked mutations
    fn iterations(&self, state: &mut S, input: &I) -> u64;

    /// Get the next mutation to apply
    fn schedule(&self, state: &mut S, input: &I) -> usize;

    /// New default implementation for mutate
    /// Implementations must forward mutate() to this method
    fn scheduled_mutate(
        &mut self,
        state: &mut S,
        input: &mut I,
        stage_idx: i32,
    ) -> Result<MutationResult, Error> {
        let mut r = MutationResult::Skipped;
        let num = self.iterations(state, input);
        for _ in 0..num {
            let idx = self.schedule(state, input);
            let outcome = self
                .mutations_mut()
                .get_and_mutate(idx, state, input, stage_idx)?;
            if outcome == MutationResult::Mutated {
                r = MutationResult::Mutated;
            }
        }
        Ok(r)
    }
}

pub struct StdScheduledMutator<I, MT, R, S>
where
    I: Input,
    MT: MutatorsTuple<I, S>,
    R: Rand,
    S: HasRand<R>,
{
    mutations: MT,
    phantom: PhantomData<(I, R, S)>,
}

impl<I, MT, R, S> Debug for StdScheduledMutator<I, MT, R, S>
where
    I: Input,
    MT: MutatorsTuple<I, S>,
    R: Rand,
    S: HasRand<R>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "StdScheduledMutator with {} mutations for Input type {}",
            self.mutations.len(),
            core::any::type_name::<I>()
        )
    }
}

impl<I, MT, R, S> Mutator<I, S> for StdScheduledMutator<I, MT, R, S>
where
    I: Input,
    MT: MutatorsTuple<I, S>,
    R: Rand,
    S: HasRand<R>,
{
    #[inline]
    fn mutate(
        &mut self,
        state: &mut S,
        input: &mut I,
        stage_idx: i32,
    ) -> Result<MutationResult, Error> {
        self.scheduled_mutate(state, input, stage_idx)
    }
}

impl<I, MT, R, S> ComposedByMutations<I, MT, S> for StdScheduledMutator<I, MT, R, S>
where
    I: Input,
    MT: MutatorsTuple<I, S>,
    R: Rand,
    S: HasRand<R>,
{
    /// Get the mutations
    #[inline]
    fn mutations(&self) -> &MT {
        &self.mutations
    }

    // Get the mutations (mut)
    #[inline]
    fn mutations_mut(&mut self) -> &mut MT {
        &mut self.mutations
    }
}

impl<I, MT, R, S> ScheduledMutator<I, MT, S> for StdScheduledMutator<I, MT, R, S>
where
    I: Input,
    MT: MutatorsTuple<I, S>,
    R: Rand,
    S: HasRand<R>,
{
    /// Compute the number of iterations used to apply stacked mutations
    fn iterations(&self, state: &mut S, _: &I) -> u64 {
        1 << (1 + state.rand_mut().below(6))
    }

    /// Get the next mutation to apply
    fn schedule(&self, state: &mut S, _: &I) -> usize {
        debug_assert!(!self.mutations().is_empty());
        state.rand_mut().below(self.mutations().len() as u64) as usize
    }
}

impl<I, MT, R, S> StdScheduledMutator<I, MT, R, S>
where
    I: Input,
    MT: MutatorsTuple<I, S>,
    R: Rand,
    S: HasRand<R>,
{
    /// Create a new StdScheduledMutator instance specifying mutations
    pub fn new(mutations: MT) -> Self {
        StdScheduledMutator {
            mutations,
            phantom: PhantomData,
        }
    }
}

/// Get the mutations that compose the Havoc mutator
pub fn havoc_mutations<C, I, R, S>() -> impl MutatorsTuple<I, S>
where
    I: Input + HasBytesVec,
    S: HasRand<R> + HasCorpus<C, I> + HasMetadata + HasMaxSize,
    C: Corpus<I>,
    R: Rand,
{
    tuple_list!(
        BitFlipMutator::new(),
        ByteFlipMutator::new(),
        ByteIncMutator::new(),
        ByteDecMutator::new(),
        ByteNegMutator::new(),
        ByteRandMutator::new(),
        ByteAddMutator::new(),
        WordAddMutator::new(),
        DwordAddMutator::new(),
        QwordAddMutator::new(),
        ByteInterestingMutator::new(),
        WordInterestingMutator::new(),
        DwordInterestingMutator::new(),
        BytesDeleteMutator::new(),
        BytesDeleteMutator::new(),
        BytesDeleteMutator::new(),
        BytesDeleteMutator::new(),
        BytesExpandMutator::new(),
        BytesInsertMutator::new(),
        BytesRandInsertMutator::new(),
        BytesSetMutator::new(),
        BytesRandSetMutator::new(),
        BytesCopyMutator::new(),
        BytesSwapMutator::new(),
        TokenInsert::new(),
        TokenReplace::new(),
        CrossoverInsertMutator::new(),
        CrossoverReplaceMutator::new(),
    )
}

//wraps around StdScheduledMutator
pub struct LoggerScheduledMutator<C, I, MT, R, S, SM>
where
    C: Corpus<I>,
    I: Input,
    MT: MutatorsTuple<I, S> + NamedTuple,
    R: Rand,
    S: HasRand<R> + HasCorpus<C, I>,
    SM: ScheduledMutator<I, MT, S>,
{
    scheduled: SM,
    mutation_log: Vec<usize>,
    phantom: PhantomData<(C, I, MT, R, S)>,
}

impl<C, I, MT, R, S, SM> Debug for LoggerScheduledMutator<C, I, MT, R, S, SM>
where
    C: Corpus<I>,
    I: Input,
    MT: MutatorsTuple<I, S> + NamedTuple,
    R: Rand,
    S: HasRand<R> + HasCorpus<C, I>,
    SM: ScheduledMutator<I, MT, S>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "LoggerScheduledMutator with {} mutations for Input type {}",
            self.scheduled.mutations().len(),
            core::any::type_name::<I>()
        )
    }
}

impl<C, I, MT, R, S, SM> Mutator<I, S> for LoggerScheduledMutator<C, I, MT, R, S, SM>
where
    C: Corpus<I>,
    I: Input,
    MT: MutatorsTuple<I, S> + NamedTuple,
    R: Rand,
    S: HasRand<R> + HasCorpus<C, I>,
    SM: ScheduledMutator<I, MT, S>,
{
    fn mutate(
        &mut self,
        state: &mut S,
        input: &mut I,
        stage_idx: i32,
    ) -> Result<MutationResult, Error> {
        self.scheduled_mutate(state, input, stage_idx)
    }

    fn post_exec(
        &mut self,
        state: &mut S,
        _stage_idx: i32,
        corpus_idx: Option<usize>,
    ) -> Result<(), Error> {
        if let Some(idx) = corpus_idx {
            let mut testcase = (*state.corpus_mut().get(idx)?).borrow_mut();
            let mut log = Vec::<String>::new();
            while let Some(idx) = self.mutation_log.pop() {
                let name = String::from(self.scheduled.mutations().get_name(idx).unwrap()); // TODO maybe return an Error on None
                log.push(name)
            }
            let meta = MutationsMetadata::new(log);
            testcase.add_metadata(meta);
        };
        // Always reset the log for each run
        self.mutation_log.clear();
        Ok(())
    }
}

impl<C, I, MT, R, S, SM> ComposedByMutations<I, MT, S>
    for LoggerScheduledMutator<C, I, MT, R, S, SM>
where
    C: Corpus<I>,
    I: Input,
    MT: MutatorsTuple<I, S> + NamedTuple,
    R: Rand,
    S: HasRand<R> + HasCorpus<C, I>,
    SM: ScheduledMutator<I, MT, S>,
{
    #[inline]
    fn mutations(&self) -> &MT {
        self.scheduled.mutations()
    }

    #[inline]
    fn mutations_mut(&mut self) -> &mut MT {
        self.scheduled.mutations_mut()
    }
}

impl<C, I, MT, R, S, SM> ScheduledMutator<I, MT, S> for LoggerScheduledMutator<C, I, MT, R, S, SM>
where
    C: Corpus<I>,
    I: Input,
    MT: MutatorsTuple<I, S> + NamedTuple,
    R: Rand,
    S: HasRand<R> + HasCorpus<C, I>,
    SM: ScheduledMutator<I, MT, S>,
{
    /// Compute the number of iterations used to apply stacked mutations
    fn iterations(&self, state: &mut S, _: &I) -> u64 {
        1 << (1 + state.rand_mut().below(6))
    }

    /// Get the next mutation to apply
    fn schedule(&self, state: &mut S, _: &I) -> usize {
        debug_assert!(!self.scheduled.mutations().is_empty());
        state
            .rand_mut()
            .below(self.scheduled.mutations().len() as u64) as usize
    }

    fn scheduled_mutate(
        &mut self,
        state: &mut S,
        input: &mut I,
        stage_idx: i32,
    ) -> Result<MutationResult, Error> {
        let mut r = MutationResult::Skipped;
        let num = self.iterations(state, input);
        self.mutation_log.clear();
        for _ in 0..num {
            let idx = self.schedule(state, input);
            self.mutation_log.push(idx);
            let outcome = self
                .mutations_mut()
                .get_and_mutate(idx, state, input, stage_idx)?;
            if outcome == MutationResult::Mutated {
                r = MutationResult::Mutated;
            }
        }
        Ok(r)
    }
}

impl<C, I, MT, R, S, SM> LoggerScheduledMutator<C, I, MT, R, S, SM>
where
    C: Corpus<I>,
    I: Input,
    MT: MutatorsTuple<I, S> + NamedTuple,
    R: Rand,
    S: HasRand<R> + HasCorpus<C, I>,
    SM: ScheduledMutator<I, MT, S>,
{
    /// Create a new StdScheduledMutator instance without mutations and corpus
    pub fn new(scheduled: SM) -> Self {
        Self {
            scheduled,
            mutation_log: vec![],
            phantom: PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        corpus::{Corpus, InMemoryCorpus, Testcase},
        inputs::{BytesInput, HasBytesVec},
        mutators::{
            mutations::SpliceMutator,
            scheduled::{havoc_mutations, StdScheduledMutator},
            Mutator,
        },
        state::State,
        utils::{Rand, StdRand, XKCDRand},
    };

    #[test]
    fn test_mut_scheduled() {
        // With the current impl, seed of 1 will result in a split at pos 2.
        let mut rand = XKCDRand::with_seed(5);
        let mut corpus: InMemoryCorpus<BytesInput> = InMemoryCorpus::new();
        corpus
            .add(Testcase::new(vec!['a' as u8, 'b' as u8, 'c' as u8]).into())
            .unwrap();
        corpus
            .add(Testcase::new(vec!['d' as u8, 'e' as u8, 'f' as u8]).into())
            .unwrap();

        let testcase = corpus.get(0).expect("Corpus did not contain entries");
        let mut input = testcase.borrow_mut().load_input().unwrap().clone();

        let mut state = State::new(rand, corpus, (), InMemoryCorpus::new(), ());

        rand.set_seed(5);

        let mut splice = SpliceMutator::new();
        splice.mutate(&mut state, &mut input, 0).unwrap();

        #[cfg(feature = "std")]
        println!("{:?}", input.bytes());

        // The pre-seeded rand should have spliced at position 2.
        // TODO: Maybe have a fixed rand for this purpose?
        assert_eq!(input.bytes(), &['a' as u8, 'b' as u8, 'f' as u8])
    }

    #[test]
    fn test_havoc() {
        // With the current impl, seed of 1 will result in a split at pos 2.
        let rand = StdRand::with_seed(0x1337);
        let mut corpus: InMemoryCorpus<BytesInput> = InMemoryCorpus::new();
        corpus
            .add(Testcase::new(vec!['a' as u8, 'b' as u8, 'c' as u8]).into())
            .unwrap();
        corpus
            .add(Testcase::new(vec!['d' as u8, 'e' as u8, 'f' as u8]).into())
            .unwrap();

        let testcase = corpus.get(0).expect("Corpus did not contain entries");
        let mut input = testcase.borrow_mut().load_input().unwrap().clone();
        let input_prior = input.clone();

        let mut state = State::new(rand, corpus, (), InMemoryCorpus::new(), ());

        let mut havoc = StdScheduledMutator::new(havoc_mutations());

        assert_eq!(input, input_prior);

        let mut equal_in_a_row = 0;

        for i in 0..42 {
            havoc.mutate(&mut state, &mut input, i).unwrap();

            // Make sure we actually mutate something, at least sometimes
            equal_in_a_row = if input == input_prior {
                equal_in_a_row + 1
            } else {
                0
            };
            assert_ne!(equal_in_a_row, 5);
        }
    }
}
