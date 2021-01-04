use alloc::boxed::Box;
use core::{ffi::c_void, ptr};

use crate::{
    corpus::Corpus,
    engines::State,
    events::EventManager,
    executors::{Executor, ExitKind, HasObservers},
    feedbacks::FeedbacksTuple,
    inputs::{HasTargetBytes, Input},
    observers::ObserversTuple,
    tuples::Named,
    utils::Rand,
    AflError,
};

use self::unix_signals::setup_crash_handlers;

/// The (unsafe) pointer to the current inmem input, for the current run.
/// This is neede for certain non-rust side effects, as well as unix signal handling.
static mut CURRENT_INPUT_PTR: *const c_void = ptr::null();
static mut CURRENT_ON_CRASH_FN: *mut Box<dyn FnMut(ExitKind, &[u8])> = ptr::null_mut();

/// The inmem executor harness
type HarnessFunction<I> = fn(&dyn Executor<I>, &[u8]) -> ExitKind;

/// The inmem executor simply calls a target function, then returns afterwards.
pub struct InMemoryExecutor<I, OT>
where
    I: Input + HasTargetBytes,
    OT: ObserversTuple,
{
    /// The name of this executor instance, to address it from other components
    name: &'static str,
    /// The harness function, being executed for each fuzzing loop execution
    harness: HarnessFunction<I>,
    /// The observers, observing each run
    observers: OT,
    /// A special function being called right before the process crashes. It may save state to restore fuzzing after respawn.
    on_crash_fn: Box<dyn FnMut(ExitKind, &[u8])>,
}

impl<I, OT> Executor<I> for InMemoryExecutor<I, OT>
where
    I: Input + HasTargetBytes,
    OT: ObserversTuple,
{
    #[inline]
    fn run_target(&mut self, input: &I) -> Result<ExitKind, AflError> {
        let bytes = input.target_bytes();
        unsafe {
            CURRENT_ON_CRASH_FN = &mut self.on_crash_fn as *mut _;
            CURRENT_INPUT_PTR = input as *const _ as *const c_void;
        }
        let ret = (self.harness)(self, bytes.as_slice());
        unsafe {
            CURRENT_ON_CRASH_FN = ptr::null_mut();
            CURRENT_INPUT_PTR = ptr::null();
        }
        Ok(ret)
    }
}

impl<I, OT> Named for InMemoryExecutor<I, OT>
where
    I: Input + HasTargetBytes,
    OT: ObserversTuple,
{
    fn name(&self) -> &str {
        self.name
    }
}

impl<I, OT> HasObservers<OT> for InMemoryExecutor<I, OT>
where
    I: Input + HasTargetBytes,
    OT: ObserversTuple,
{
    #[inline]
    fn observers(&self) -> &OT {
        &self.observers
    }

    #[inline]
    fn observers_mut(&mut self) -> &mut OT {
        &mut self.observers
    }
}

impl<I, OT> InMemoryExecutor<I, OT>
where
    I: Input + HasTargetBytes,
    OT: ObserversTuple,
{
    /// Create a new in mem executor.
    /// * `name` - the name of this executor (to address it along the way)
    /// * `harness_fn` - the harness, executiong the function
    /// * `on_crash_fn` - When an in-mem harness crashes, it may safe some state to continue fuzzing later.
    ///                   Do that that in this function. The program will crash afterwards.
    /// * `observers` - the observers observing the target during execution
    pub fn new<C, E, EM, FT, R>(
        name: &'static str,
        harness_fn: HarnessFunction<I>,
        observers: OT,
        on_crash_fn: Box<dyn FnMut(ExitKind, &[u8])>,
        state: &State<I, R, FT, OT>,
        corpus: &C,
        event_manager: &mut EM,
    ) -> Self
    where
        C: Corpus<I, R>,
        E: Executor<I>,
        EM: EventManager<C, E, OT, FT, I, R>,
        FT: FeedbacksTuple<I>,
        R: Rand,
    {
        unsafe {
            CORPUS_PTR = corpus as *const _ as *const c_void;
            STATE_PTR = state as *const _ as *const c_void;

            setup_crash_handlers(event_manager);
        }

        Self {
            harness: harness_fn,
            on_crash_fn,
            observers,
            name,
        }
    }
}

static mut CORPUS_PTR: *const c_void = ptr::null_mut();
static mut STATE_PTR: *const c_void = ptr::null_mut();

/// Serialize the current state and corpus during an executiont to bytes.
/// This method is needed when the fuzzer run crashes and has to restart.
pub unsafe fn serialize_state_corpus<C, FT, I, OT, R>() -> Result<Vec<u8>, AflError>
where
    C: Corpus<I, R>,
    FT: FeedbacksTuple<I>,
    I: Input,
    OT: ObserversTuple,
    R: Rand,
{
    if STATE_PTR.is_null() || CORPUS_PTR.is_null() {
        return Err(AflError::IllegalState(
            "State or corpus is not currently set and cannot be serialized in in-mem-executor"
                .to_string(),
        ));
    }
    let state: &State<I, R, FT, OT> = (STATE_PTR as *const State<I, R, FT, OT>).as_ref().unwrap();
    let corpus = (CORPUS_PTR as *mut C).as_ref().unwrap();
    let state_bytes = postcard::to_allocvec(&state)?;
    let corpus_bytes = postcard::to_allocvec(&corpus)?;
    Ok(postcard::to_allocvec(&(state_bytes, corpus_bytes))?)
}

/// Deserialize the state and corpus tuple, previously serialized with `serialize_state_corpus(...)`
pub fn deserialize_state_corpus<C, FT, I, OT, R>(
    state_corpus_serialized: &[u8],
) -> Result<(State<I, R, FT, OT>, C), AflError>
where
    C: Corpus<I, R>,
    FT: FeedbacksTuple<I>,
    I: Input,
    OT: ObserversTuple,
    R: Rand,
{
    let tuple: (Vec<u8>, Vec<u8>) = postcard::from_bytes(&state_corpus_serialized)?;
    Ok((
        postcard::from_bytes(&tuple.0)?,
        postcard::from_bytes(&tuple.1)?,
    ))
}

#[cfg(feature = "std")]
#[cfg(unix)]
pub mod unix_signals {

    extern crate libc;

    // Unhandled signals: SIGALRM, SIGHUP, SIGINT, SIGKILL, SIGQUIT, SIGTERM
    use libc::{
        c_int, c_void, sigaction, siginfo_t, SA_NODEFER, SA_SIGINFO, SIGABRT, SIGBUS, SIGFPE,
        SIGILL, SIGPIPE, SIGSEGV, SIGUSR2,
    };

    use std::{
        io::{stdout, Write}, // Write brings flush() into scope
        mem,
        process,
        ptr,
    };

    use crate::{
        corpus::Corpus,
        events::EventManager,
        executors::{
            inmemory::{serialize_state_corpus, ExitKind, CURRENT_INPUT_PTR, CURRENT_ON_CRASH_FN},
            Executor,
        },
        feedbacks::FeedbacksTuple,
        inputs::Input,
        observers::ObserversTuple,
        utils::Rand,
    };

    static mut EVENT_MANAGER_PTR: *mut c_void = ptr::null_mut();

    pub unsafe extern "C" fn libaflrs_executor_inmem_handle_crash<EM, C, E, OT, FT, I, R>(
        _sig: c_int,
        info: siginfo_t,
        _void: c_void,
    ) where
        EM: EventManager<C, E, OT, FT, I, R>,
        C: Corpus<I, R>,
        E: Executor<I>,
        OT: ObserversTuple,
        FT: FeedbacksTuple<I>,
        I: Input,
        R: Rand,
    {
        if CURRENT_INPUT_PTR == ptr::null() {
            println!(
                "We died accessing addr {}, but are not in client...",
                info.si_addr() as usize
            );
        }

        #[cfg(feature = "std")]
        println!("Child crashed!");
        #[cfg(feature = "std")]
        let _ = stdout().flush();

        let input = (CURRENT_INPUT_PTR as *const I).as_ref().unwrap();
        let manager = (EVENT_MANAGER_PTR as *mut EM).as_mut().unwrap();

        manager.crash(input).expect("Error in sending Crash event");

        if !CURRENT_ON_CRASH_FN.is_null() {
            (*CURRENT_ON_CRASH_FN)(
                ExitKind::Crash,
                &serialize_state_corpus::<C, FT, I, OT, R>().unwrap(),
            );
        }

        std::process::exit(139);
    }

    pub unsafe extern "C" fn libaflrs_executor_inmem_handle_timeout<EM, C, E, OT, FT, I, R>(
        _sig: c_int,
        _info: siginfo_t,
        _void: c_void,
    ) where
        EM: EventManager<C, E, OT, FT, I, R>,
        C: Corpus<I, R>,
        E: Executor<I>,
        OT: ObserversTuple,
        FT: FeedbacksTuple<I>,
        I: Input,
        R: Rand,
    {
        dbg!("TIMEOUT/SIGUSR2 received");
        if CURRENT_INPUT_PTR.is_null() {
            dbg!("TIMEOUT or SIGUSR2 happened, but currently not fuzzing.");
            return;
        }

        let input = (CURRENT_INPUT_PTR as *const I).as_ref().unwrap();
        let manager = (EVENT_MANAGER_PTR as *mut EM).as_mut().unwrap();

        manager
            .timeout(input)
            .expect("Error in sending Timeout event");

        if !CURRENT_ON_CRASH_FN.is_null() {
            (*CURRENT_ON_CRASH_FN)(
                ExitKind::Timeout,
                &serialize_state_corpus::<C, FT, I, OT, R>().unwrap(),
            );
        }

        // TODO: send LLMP.
        println!("Timeout in fuzz run.");
        let _ = stdout().flush();
        process::abort();
    }

    // TODO clearly state that manager should be static (maybe put the 'static lifetime?)
    pub unsafe fn setup_crash_handlers<EM, C, E, OT, FT, I, R>(manager: &mut EM)
    where
        EM: EventManager<C, E, OT, FT, I, R>,
        C: Corpus<I, R>,
        E: Executor<I>,
        OT: ObserversTuple,
        FT: FeedbacksTuple<I>,
        I: Input,
        R: Rand,
    {
        EVENT_MANAGER_PTR = manager as *mut _ as *mut c_void;

        let mut sa: sigaction = mem::zeroed();
        libc::sigemptyset(&mut sa.sa_mask as *mut libc::sigset_t);
        sa.sa_flags = SA_NODEFER | SA_SIGINFO;
        sa.sa_sigaction = libaflrs_executor_inmem_handle_crash::<EM, C, E, OT, FT, I, R> as usize;
        for (sig, msg) in &[
            (SIGSEGV, "segfault"),
            (SIGBUS, "sigbus"),
            (SIGABRT, "sigabrt"),
            (SIGILL, "illegal instruction"),
            (SIGFPE, "fp exception"),
            (SIGPIPE, "pipe"),
        ] {
            if sigaction(*sig, &mut sa as *mut sigaction, ptr::null_mut()) < 0 {
                panic!("Could not set up {} handler", &msg);
            }
        }

        sa.sa_sigaction = libaflrs_executor_inmem_handle_timeout::<EM, C, E, OT, FT, I, R> as usize;
        if sigaction(SIGUSR2, &mut sa as *mut sigaction, ptr::null_mut()) < 0 {
            panic!("Could not set up sigusr2 handler for timeouts");
        }
    }
}

//#[cfg(feature = "std")]
//#[cfg(unix)]
//use unix_signals as os_signals;
//#[cfg(feature = "std")]
//#[cfg(not(unix))]
//compile_error!("InMemoryExecutor not yet supported on this OS");

#[cfg(test)]
mod tests {

    use alloc::boxed::Box;

    use crate::executors::inmemory::InMemoryExecutor;
    use crate::executors::{Executor, ExitKind};
    use crate::inputs::{HasTargetBytes, Input, TargetBytes};
    use crate::tuples::tuple_list;

    use serde::{Deserialize, Serialize};

    #[derive(Clone, Serialize, Deserialize, Debug)]
    struct NopInput {}
    impl Input for NopInput {}
    impl HasTargetBytes for NopInput {
        fn target_bytes(&self) -> TargetBytes {
            TargetBytes::Owned(vec![0])
        }
    }

    #[cfg(feature = "std")]
    fn test_harness_fn_nop(_executor: &dyn Executor<NopInput>, buf: &[u8]) -> ExitKind {
        println!("Fake exec with buf of len {}", buf.len());
        ExitKind::Ok
    }

    #[cfg(not(feature = "std"))]
    fn test_harness_fn_nop(_executor: &dyn Executor<NopInput>, _buf: &[u8]) -> ExitKind {
        ExitKind::Ok
    }

    #[test]
    fn test_inmem_exec() {
        /*
        let mut in_mem_executor =
            InMemoryExecutor::new("main", test_harness_fn_nop, tuple_list!(), Box::new(|_| ()));
        let mut input = NopInput {};
        assert!(in_mem_executor.run_target(&mut input).is_ok());
        */
    }
}
