use std::time::Instant;

// ============================================================================
// Turn Virtual Machine Stress Tests (Phase 2)
//
// These tests avoid anti-patterns by strictly interacting with the standard
// Runner API, rather than attempting to mutate VM internals directly. They
// assert that the Tokio scheduler and Turn VM process model remain stable
// under massive concurrent load.
// ============================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_the_swarm_10k_concurrent_agents() {
    let script = r#"
        let agent = turn() -> Num {
            return 1;
        };

        persist let count = 0;
        persist let spawned = 0;

        while count < 10000 {
            let pid = spawn linked agent;
            persist let count = count + 1;
            persist let spawned = spawned + 1;
        }

        return spawned;
    "#;

    println!("Starting Swarm Test: 10,000 Concurrent Agents...");
    let start = Instant::now();
    let result = turn::run(script);
    let duration = start.elapsed();

    assert!(result.is_ok(), "The Swarm panicked the VM: {:?}", result.err());
    
    // Verify it completely executed and returned 10000
    let res_val = result.unwrap();
    assert_eq!(res_val, turn::value::Value::Num(10000.0));
    println!("Swarm Test Passed in {:?}", duration);
}

#[tokio::test]
async fn test_mailbox_flood() {
    let script = r#"
        let worker = turn() -> Num {
            persist let count = 0;
            while count < 5000 {
                let msg = await receive;
                if msg == "ping" {
                    persist let count = count + 1;
                }
            }
            return count;
        };

        let target_pid = spawn linked worker;
        
        // Bombard the mailbox faster than it processes them
        persist let sent = 0;
        while sent < 5000 {
            send target_pid, "ping";
            persist let sent = sent + 1;
        }

        persist let res = null;
        while res == null {
            persist let res = harvest;
        }

        return res["reason"];
    "#;

    println!("Starting Mailbox Flood Test: 5,000 asynchronous messages...");
    let start = Instant::now();
    // Use a tighter loop to ensure we're not timing out CI runners endlessly,
    // 5000 messages is enough to verify queue memory bounds without hanging.
    let result = turn::run(script);
    let duration = start.elapsed();

    assert!(result.is_ok(), "Mailbox flooding panicked the VM: {:?}", result.err());
    
    let res_val = result.unwrap();
    assert_eq!(res_val, turn::value::Value::Num(5000.0));
    println!("Mailbox Flood Test Passed in {:?}", duration);
}
