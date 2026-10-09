# Initial decisions

- Scope: repository bootstrap, plain-text core, status and task CLI only. No GTK, live desktop changes, or publication in this phase.
- Environment inspected: Rust/Cargo 1.98.1, GTK4 4.22.5, libadwaita 1.9.4; ryoku and ryoku-shell are installed. Actual widget API inspection deferred to adapter phase.
- Repository directory: ZFG; package/binary: zfg. Public app ID and GitHub owner remain undecided.
- License: MIT as requested by the implementation specification. Copyright ownership to be completed before publication.
- Five-task capacity is fixed in this phase; settings configurability follows later.
- Advisory fs2 lock coordinates app processes. Atomic replacement handles each list; a durable redo journal handles interrupted cross-list mutations. Lists remain canonical after recovery.
- Destructive mutations compare full ordered snapshots, rather than identifying duplicate tasks by text.
- External manual edits are reread, but editors not participating in locking can race with app writes. No claim of atomic visibility across two files or protection against uncooperative simultaneous writers.
- No Git commits or branches created. The provided directory was not a Git repository.
