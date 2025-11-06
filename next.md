  pub enum ReadFrom {
      Local,   // Read from this node's state machine
      Leader,  // Forward to leader, read from leader's state machine
  }

  pub enum ReadConsistency {
      Stale,              // No synchronization, just read
      LeaderLease,        // Leader uses lease (no quorum check)
      Linearizable,       // Leader checks quorum (ReadIndex)
  }

  pub struct ReadOptions {
      from: ReadFrom,
      consistency: ReadConsistency,
  }

  The Matrix

  |              | Local                                                        | Leader                                                    |
  |--------------|--------------------------------------------------------------|-----------------------------------------------------------|
  | Stale        | Read local SM immediately ✅ eventual_read()                  | Leader reads its SM immediately (rare, but valid)         |
  | LeaderLease  | Get lease linearizer, await_ready(), read local              | Leader uses lease, reads its SM                           |
  | Linearizable | Get ReadIndex linearizer, await_ready(), read local ✅ read() | Leader uses ReadIndex, reads its SM (also in your read()) |

  Your Observations

  "Stale Leader doesn't really make much sense"

  Actually it does make sense and you nailed why:

  // ReadFrom::Leader + ReadConsistency::Stale
  // Forward to leader, leader reads its state machine WITHOUT await_ready()

  vs.

  // ReadFrom::Leader + ReadConsistency::LeaderLease  
  // Forward to leader, leader does await_ready() on lease linearizer, then reads

  The difference:
  - Stale Leader: Leader's state machine might be behind its raft log (applied_index < commit_index)
  - Lease/Linearizable Leader: Leader waits for state machine to catch up

  So Stale Leader = "give me whatever the leader has in RAM right now, I don't care if it's slightly behind the commit log"

  When Leader/Follower Matters

  match (is_leader, read_from, consistency) {
      // I'm leader, read local - all combos make sense
      (true, Local, Stale) => { /* fast */ },
      (true, Local, LeaderLease) => { /* await_ready with lease */ },
      (true, Local, Linearizable) => { /* await_ready with ReadIndex */ },

      // I'm leader, but client asked to read from "leader" (no-op forward)
      (true, Leader, _) => { /* same as Local, already leader */ },

      // I'm follower, read local - makes sense with linearizer
      (false, Local, Stale) => { /* eventual_read */ },
      (false, Local, LeaderLease) => { /* get lease lin, await_ready, read */ },
      (false, Local, Linearizable) => { /* get ReadIndex lin, await_ready, read */ },

      // I'm follower, forward to leader
      (false, Leader, _) => { /* forward entire read to leader */ },
  }

  Cleaner API

  impl DistKV {
      pub async fn get_with_options(
          &self,
          key: String,
          from: ReadFrom,
          consistency: ReadConsistency,
      ) -> Result<String, ...>

      // Convenience methods
      pub async fn get(&self, key: String) -> Result<String, ...> {
          self.get_with_options(key, ReadFrom::Local, ReadConsistency::Linearizable).await
      }

      pub async fn get_eventual(&self, key: String) -> Result<String, ...> {
          self.get_with_options(key, ReadFrom::Local, ReadConsistency::Stale).await
      }
  }