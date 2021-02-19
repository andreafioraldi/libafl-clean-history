use crate::inputs::HasBytesVec;
use alloc::vec::Vec;
use core::{fmt, default::Default, marker::PhantomData};
use fmt::Debug;

use crate::{
    inputs::{Input},
    mutators::{Mutator, HasMaxSize, DEFAULT_MAX_SIZE},
    state::{HasRand, HasCorpus, HasMetadata},
    corpus::Corpus,
    utils::Rand,
    Error,
};

pub use crate::mutators::mutations::*;
pub use crate::mutators::token_mutations::*;

pub trait ScheduledMutator<I, S>: Mutator<I> + ComposedByMutations<I, S>
where
    I: Input,
{
    /// Compute the number of iterations used to apply stacked mutations
    fn iterations(&mut self, state: &mut S, input: &I) -> u64;
    //{
    //    1 << (1 + state.rand_mut().below(6))
    //}

    /// Get the next mutation to apply
    fn schedule(&mut self, mutations_count: usize, state: &mut S, input: &I) -> usize;
    //{
    //    debug_assert!(mutations_count > 0);
    //    rand.below(mutations_count as u64) as usize
    //}

    /// New default implementation for mutate
    /// Implementations must forward mutate() to this method
    fn scheduled_mutate(
        &mut self,
        state: &mut S,
        input: &mut I,
        _stage_idx: i32,
    ) -> Result<(), Error> {
        let num = self.iterations(state, input);
        for _ in 0..num {
            let idx = self.schedule(self.mutations_count(), state, input);
            self.mutation_by_idx(idx)(self, state, input)?;
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct StdScheduledMutator<I, R, S>
where
I: Input,
R: Rand,
S: HasRand<R>,{
    mutations: Vec<MutationFunction<I, Self, S>>,
    max_size: usize,
}

impl<I, R, S> Debug for StdScheduledMutator<I, R, S>
where
    I: Input,
    R: Rand,
    S: HasRand<R>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "StdScheduledMutator with {} Mutations, max_size: {}, for Input type {}",
            self.mutations.len(),
            self.max_size,
            core::any::type_name::<I>()
        )
    }
}

impl<I, R, S> Mutator<I> for StdScheduledMutator<I, R, S>
where
    I: Input,
    R: Rand,
    S: HasRand<R>,
{
    fn mutate(
        &mut self,
        rand: &mut R,
        state: &mut S,
        input: &mut I,
        _stage_idx: i32,
    ) -> Result<(), Error> {
        self.scheduled_mutate(state, input, _stage_idx)
    }
}

impl<I, R, S> ComposedByMutations<I, S> for StdScheduledMutator<I, R, S>
where
    I: Input,
    R: Rand,
    S: HasRand<R>,
{
    #[inline]
    fn mutation_by_idx(&self, index: usize) -> MutationFunction<I, Self, S> {
        self.mutations[index]
    }

    #[inline]
    fn mutations_count(&self) -> usize {
        self.mutations.len()
    }

    #[inline]
    fn add_mutation(&mut self, mutation: MutationFunction<I, Self, S>) {
        self.mutations.push(mutation)
    }
}

impl<I, R, S> ScheduledMutator<I, S> for StdScheduledMutator<I, R, S>
where
    I: Input,
    R: Rand,
    S: HasRand<R>,
{
    // Just use the default methods
}

impl<I, R, S> HasMaxSize for StdScheduledMutator<I, R, S>
where
    I: Input,
    R: Rand,
    S: HasRand<R>,
{
    #[inline]
    fn max_size(&self) -> usize {
        self.max_size
    }

    #[inline]
    fn set_max_size(&mut self, max_size: usize) {
        self.max_size = max_size;
    }
}

impl<I, R, S> StdScheduledMutator<I, R, S>
where
    I: Input,
    R: Rand,
    S: HasRand<R>,
{
    /// Create a new StdScheduledMutator instance without mutations and corpus
    pub fn new() -> Self {
        Self {
            mutations: vec![],
            max_size: DEFAULT_MAX_SIZE,
        }
    }

    /// Create a new StdScheduledMutator instance specifying mutations
    pub fn with_mutations(mutations: Vec<MutationFunction<I, Self, S>>) -> Self {
        StdScheduledMutator {
            mutations: mutations,
            max_size: DEFAULT_MAX_SIZE,
        }
    }
}

/// Schedule some selected byte level mutations given a ScheduledMutator type
#[derive(Clone, Debug)]
pub struct HavocBytesMutator<C, I, R, S, SM>
where
    SM: ScheduledMutator<I, S> + HasMaxSize,
    I: Input + HasBytesVec,
    S: HasRand<R> + HasCorpus<C, I> + HasMetadata,
    C: Corpus<I>,
    R: Rand,
{
    scheduled: SM,
    phantom: PhantomData<(C, I, R, S)>,
}

impl<C, I, R, S, SM> Mutator<I> for HavocBytesMutator<C, I, R, S, SM>
where
    SM: ScheduledMutator<I, S> + HasMaxSize,
    I: Input + HasBytesVec,
    S: HasRand<R> + HasCorpus<C, I> + HasMetadata,
    C: Corpus<I>,
    R: Rand,
{
    /// Mutate bytes
    fn mutate(
        &mut self,
        rand: &mut R,
        state: &mut S,
        input: &mut I,
        stage_idx: i32,
    ) -> Result<(), Error> {
        self.scheduled.mutate(state, input, stage_idx)?;
        /*let num = self.scheduled.iterations(state, input);
        for _ in 0..num {
            let idx = self.scheduled.schedule(14, state, input);
            let mutation = match idx {
                0 => mutation_bitflip,
                1 => mutation_byteflip,
                2 => mutation_byteinc,
                3 => mutation_bytedec,
                4 => mutation_byteneg,
                5 => mutation_byterand,

                6 => mutation_byteadd,
                7 => mutation_wordadd,
                8 => mutation_dwordadd,
                9 => mutation_byteinteresting,
                10 => mutation_wordinteresting,
                11 => mutation_dwordinteresting,
                _ => mutation_splice,
            };
            mutation(self, state, input)?;
        }*/
        Ok(())
    }
}

impl<C, I, R, S, SM> HasMaxSize for HavocBytesMutator<C, I, R, S, SM>
where
    SM: ScheduledMutator<I, S> + HasMaxSize,
    I: Input + HasBytesVec,
    S: HasRand<R> + HasCorpus<C, I> + HasMetadata,
    C: Corpus<I>,
    R: Rand,
{
    #[inline]
    fn max_size(&self) -> usize {
        self.scheduled.max_size()
    }

    #[inline]
    fn set_max_size(&mut self, max_size: usize) {
        self.scheduled.set_max_size(max_size);
    }
}

impl<C, I, R, S, SM> HavocBytesMutator<C, I, R, S, SM>
where
    SM: ScheduledMutator<I, S> + HasMaxSize,
    I: Input + HasBytesVec,
    S: HasRand<R> + HasCorpus<C, I> + HasMetadata,
    C: Corpus<I>,
    R: Rand,
{
    /// Create a new HavocBytesMutator instance given a ScheduledMutator to wrap
    pub fn new(mut scheduled: SM) -> Self {
        scheduled.add_mutation(mutation_bitflip);
        scheduled.add_mutation(mutation_splice);
        Self {
            scheduled: scheduled,
            phantom: PhantomData,
        }
    }
}

impl<C, I, R, S> Default for HavocBytesMutator<C, I, R, S, StdScheduledMutator<I, R, S>>
where
    I: Input + HasBytesVec,
    S: HasRand<R> + HasCorpus<C, I> + HasMetadata,
    C: Corpus<I>,
    R: Rand,
{
    /// Create a new HavocBytesMutator instance wrapping StdScheduledMutator
    fn default() -> Self {
        let mut scheduled = StdScheduledMutator::<I, R, S>::new();
        scheduled.add_mutation(mutation_bitflip);
        scheduled.add_mutation(mutation_byteflip);
        scheduled.add_mutation(mutation_byteinc);
        scheduled.add_mutation(mutation_bytedec);
        scheduled.add_mutation(mutation_byteneg);
        scheduled.add_mutation(mutation_byterand);

        scheduled.add_mutation(mutation_byteadd);
        scheduled.add_mutation(mutation_wordadd);
        scheduled.add_mutation(mutation_dwordadd);
        scheduled.add_mutation(mutation_qwordadd);
        scheduled.add_mutation(mutation_byteinteresting);
        scheduled.add_mutation(mutation_wordinteresting);
        scheduled.add_mutation(mutation_dwordinteresting);

        scheduled.add_mutation(mutation_bytesdelete);
        scheduled.add_mutation(mutation_bytesdelete);
        scheduled.add_mutation(mutation_bytesdelete);
        scheduled.add_mutation(mutation_bytesdelete);
        scheduled.add_mutation(mutation_bytesexpand);
        scheduled.add_mutation(mutation_bytesinsert);
        scheduled.add_mutation(mutation_bytesrandinsert);
        scheduled.add_mutation(mutation_bytesset);
        scheduled.add_mutation(mutation_bytesrandset);
        scheduled.add_mutation(mutation_bytescopy);
        scheduled.add_mutation(mutation_bytesswap);

        scheduled.add_mutation(mutation_tokeninsert);
        scheduled.add_mutation(mutation_tokenreplace);

        scheduled.add_mutation(mutation_crossover_insert);
        scheduled.add_mutation(mutation_crossover_replace);
        //scheduled.add_mutation(mutation_splice);

        HavocBytesMutator {
            scheduled: scheduled,
            phantom: PhantomData,
        }
    }
}

/*
#[cfg(test)]
mod tests {
    use crate::{
        corpus::{Corpus, InMemoryCorpus, Testcase},
        inputs::{BytesInput, HasBytesVec},
        mutators::{
            scheduled::{mutation_splice, HavocBytesMutator, StdScheduledMutator},
            Mutator,
        },
        state::State,
        utils::{Rand, StdRand, XKCDRand},
    };

    #[test]
    fn test_mut_scheduled() {
        // With the current impl, seed of 1 will result in a split at pos 2.
        let mut rand = XKCDRand::new();
        let mut corpus: InMemoryCorpus<BytesInput, _> = InMemoryCorpus::new();
        corpus.add(Testcase::new(vec!['a' as u8, 'b' as u8, 'c' as u8]).into());
        corpus.add(Testcase::new(vec!['d' as u8, 'e' as u8, 'f' as u8]).into());

        let (testcase, _) = corpus
            .next(&mut rand)
            .expect("Corpus did not contain entries");
        let mut input = testcase.borrow_mut().load_input().unwrap().clone();

        let mut state = State::new(corpus, (), InMemoryCorpus::new(), ());

        rand.set_seed(5);

        let mut mutator = StdScheduledMutator::<
            InMemoryCorpus<BytesInput, XKCDRand>,
            _,
            _,
            State<_, (), _, InMemoryCorpus<BytesInput, XKCDRand>, (), _>,
        >::new();

        mutation_splice(&mut mutator, &mut rand, &mut state, &mut input).unwrap();

        #[cfg(feature = "std")]
        println!("{:?}", input.bytes());

        // The pre-seeded rand should have spliced at position 2.
        // TODO: Maybe have a fixed rand for this purpose?
        assert_eq!(input.bytes(), &['a' as u8, 'b' as u8, 'f' as u8])
    }

    #[test]
    fn test_havoc() {
        // With the current impl, seed of 1 will result in a split at pos 2.
        let mut rand = StdRand::new(0x1337);
        let mut corpus: InMemoryCorpus<BytesInput, StdRand> = InMemoryCorpus::new();
        corpus.add(Testcase::new(vec!['a' as u8, 'b' as u8, 'c' as u8]).into());
        corpus.add(Testcase::new(vec!['d' as u8, 'e' as u8, 'f' as u8]).into());

        let (testcase, _) = corpus
            .next(&mut rand)
            .expect("Corpus did not contain entries");
        let mut input = testcase.borrow_mut().load_input().unwrap().clone();
        let input_prior = input.clone();

        let mut state = State::new(corpus, (), InMemoryCorpus::new(), ());

        let mut havoc = HavocBytesMutator::new(StdScheduledMutator::new());

        assert_eq!(input, input_prior);

        for i in 0..42 {
            havoc.mutate(&mut rand, &mut state, &mut input, i).unwrap();
            assert_ne!(input, input_prior);
        }
    }
}
*/