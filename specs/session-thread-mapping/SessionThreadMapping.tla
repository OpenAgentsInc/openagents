----------------------------- MODULE SessionThreadMapping -----------------------------
EXTENDS FiniteSets, TLC

CONSTANTS Sessions, Threads

ASSUME Sessions # {}
ASSUME Threads # {}

None == "none"

VARIABLES sessionThread, threadSession, persisted, crashed

vars == << sessionThread, threadSession, persisted, crashed >>

Init ==
  /\ sessionThread = [s \in Sessions |-> None]
  /\ threadSession = [t \in Threads |-> None]
  /\ persisted = {}
  /\ crashed = FALSE

Pair(s, t) == <<s, t>>

Bind(s, t) ==
  /\ ~crashed
  /\ sessionThread[s] = None
  /\ threadSession[t] = None
  /\ sessionThread' = [sessionThread EXCEPT ![s] = t]
  /\ threadSession' = [threadSession EXCEPT ![t] = s]
  /\ persisted' = persisted \cup {Pair(s, t)}
  /\ UNCHANGED crashed

ArchiveSession(s) ==
  /\ ~crashed
  /\ sessionThread[s] # None
  /\ LET t == sessionThread[s] IN
     /\ sessionThread' = [sessionThread EXCEPT ![s] = None]
     /\ threadSession' = [threadSession EXCEPT ![t] = None]
     /\ persisted' = persisted \ {Pair(s, t)}
  /\ UNCHANGED crashed

Crash ==
  /\ ~crashed
  /\ crashed' = TRUE
  /\ UNCHANGED << sessionThread, threadSession, persisted >>

Reload ==
  /\ crashed
  /\ sessionThread' = [s \in Sessions |-> IF \E t \in Threads : Pair(s, t) \in persisted
                                      THEN CHOOSE t \in Threads : Pair(s, t) \in persisted
                                      ELSE None]
  /\ threadSession' = [t \in Threads |-> IF \E s \in Sessions : Pair(s, t) \in persisted
                                      THEN CHOOSE s \in Sessions : Pair(s, t) \in persisted
                                      ELSE None]
  /\ crashed' = FALSE
  /\ UNCHANGED persisted

Next ==
  \/ \E s \in Sessions, t \in Threads : Bind(s, t)
  \/ \E s \in Sessions : ArchiveSession(s)
  \/ Crash
  \/ Reload

Spec == Init /\ [][Next]_vars /\ WF_vars(Next)

NoOrphanThreadBinding ==
  /\ \A s \in Sessions : sessionThread[s] = None \/ threadSession[sessionThread[s]] = s
  /\ \A t \in Threads : threadSession[t] = None \/ sessionThread[threadSession[t]] = t

NoDoubleBind ==
  /\ \A s1, s2 \in Sessions :
      /\ s1 # s2
      /\ sessionThread[s1] # None
      => sessionThread[s1] # sessionThread[s2]
  /\ \A t1, t2 \in Threads :
      /\ t1 # t2
      /\ threadSession[t1] # None
      => threadSession[t1] # threadSession[t2]

PersistedMappingConsistent ==
  \A p \in persisted :
    /\ p[1] \in Sessions
    /\ p[2] \in Threads
    /\ sessionThread[p[1]] = p[2]
    /\ threadSession[p[2]] = p[1]

CrashReloadEventuallyRestoresBindings ==
  [](crashed => <> ~crashed)

================================================================================
