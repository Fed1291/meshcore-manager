//! Jobs side by side: a backup, a restore, a commit, a reading, each on its own thread, so
//! several boards are worked on at once. What keeps that safe:
//! - **One job per board.** A job first claims its board (`Claims::take`): by its USB serial
//!   number when it has a real one, else by its port (`Port::same_board`), so a board that drops
//!   off and comes back on another port (a restore that restarts it) is still the same claim. A
//!   job for a claimed board waits in the worker's queue until the claim is gone.
//! - **The scan leaves claimed boards alone**: it never opens their port, never asks them.
//! - **A claim always ends**: it is dropped when the job ends, also when the job fails or
//!   panics (`Drop`), and a poisoned lock (a thread that panicked holding it) is still used.
//! - **The operating system says no too**: a serial port opens for one handle at a time (Windows:
//!   the driver's exclusive flag; Linux: `serialport` sets TIOCEXCL and a `flock`), so even a
//!   mistake here cannot make two jobs talk into one board.
//! - Files each job writes are its own (a new backup folder made in one step, a journal per
//!   board), written to a temporary name, synced, then renamed.

use std::sync::{Arc, Mutex, MutexGuard};

use crate::serial::Port;

/// A job's number, to tell its progress and its end from the others'
pub type JobId = u64;

/// The boards running jobs hold, shared by the worker and its job threads
#[derive(Clone, Default)]
pub struct Claims(Arc<Mutex<Vec<(JobId, Port)>>>);

impl Claims {
    /// The list, even after a job panicked holding the lock (the list itself stays right: each
    /// change is one push or one retain)
    fn list(&self) -> MutexGuard<'_, Vec<(JobId, Port)>> {
        self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// A job holds this board
    pub fn busy(&self, port: &Port) -> bool {
        self.list().iter().any(|(_, p)| p.same_board(port))
    }

    /// Claims the board for job `id`; None when another job holds it (checked and taken in one
    /// step: two jobs cannot both get it)
    pub fn take(&self, id: JobId, port: &Port) -> Option<Claim> {
        let mut list = self.list();
        if list.iter().any(|(_, p)| p.same_board(port)) {
            return None;
        }
        list.push((id, port.clone()));
        Some(Claim { claims: self.clone(), id })
    }

    /// How many jobs run
    pub fn count(&self) -> usize {
        self.list().len()
    }
}

/// A board held by a job, until this is dropped (the job ended, however)
pub struct Claim {
    claims: Claims,
    id: JobId,
}

impl Drop for Claim {
    fn drop(&mut self) {
        let id = self.id;
        self.claims.list().retain(|(job, _)| *job != id);
    }
}
