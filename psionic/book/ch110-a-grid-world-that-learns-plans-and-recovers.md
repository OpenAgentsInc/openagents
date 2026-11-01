# 110. A grid world that learns, plans and recovers

> **Status: partly implemented.**

This chapter builds a small headless grid world and three ways to act in it.
An exact planner computes the best action in every cell. A Q-learner finds
the same answer from seeded episodes, without ever reading the planner. An
agent with a learned table of transitions plans, takes one step, writes down
what happened and plans again. Then a wall moves, and we measure in steps
which of them recovers. A planted map shows the planner trusting a wrong
model and the cost of doing so.

What runs is the tabular core: maps of at most 8 by 8 cells, exact value
iteration, Q-learning, replanning on a learned transition table, and a
measured model error. Behavior cloning and DAgger with
[chapter 103](ch103-learning-with-arrays.md)'s trainer, planning with
[chapter 109](ch109-learn-a-physical-system.md)'s learned continuous model,
and suites of held-out maps are not built yet; the last section lists them.

```shell
coax/bin/coax run psionic/examples/grid-world.coax
coax/bin/coax test psionic/lib/gridworld.coax
```

```text
profile: cpu-f64-grid-v1
hand map S#G/... at discount 0.5
  sweep 1: S -1.0 | row 2: -1.0 -1.0 -1.0 | residual 1.0
  sweep 2: S -1.5 | row 2: -1.5 -1.5 -1.0 | residual 0.5
  sweep 3: S -1.75 | row 2: -1.75 -1.5 -1.0 | residual 0.25
  sweep 4: S -1.875 | row 2: -1.75 -1.5 -1.0 | residual 0.125
  sweep 5: S -1.875 | row 2: -1.75 -1.5 -1.0 | residual 0.0
    v#G
    >>^
exhaustive search: 256 policies, best values equal the planner's: true
map A, discount 0.9: 5 steps from the start
    >>>v<
    #v#v#
    >>>>v
    >>>>G
Q-learning on A: 1000 episodes of at most 64 steps, alpha 1, explore 8/16, exploring starts, tolerance 1e-9
  seed 0: shortest walk after 13 episodes (409 steps); within tolerance after 116 episodes (1172 steps); pairs off 0; replay digest equal
  seed 1: shortest walk after 30 episodes (478 steps); within tolerance after 73 episodes (857 steps); pairs off 0; replay digest equal
  seed 2: shortest walk after 20 episodes (379 steps); within tolerance after 189 episodes (1746 steps); pairs off 0; replay digest equal
  seed 3: shortest walk after 15 episodes (321 steps); within tolerance after 91 episodes (882 steps); pairs off 0; replay digest equal
the wall moves: map B closes the gap at column 3 (9 steps)
    >v<<<
    #v###
    >>>>v
    >>>>G
  A's greedy Q policy, fixed: reached false, 200 steps, 199 collisions, promised -1, 0 decisions, 0 sweeps
  replanning on A's learned model: reached true, 10 steps, 1 collisions, promised 5, 10 decisions, 96 sweeps
  surprises 1, sweeps per decision 9.6
  planning on B's true table: reached true, 9 steps, 0 collisions, promised 9, 9 decisions, 90 sweeps
  Q-learning resumed on B, seed 0: shortest walk again after 52 episodes (461 steps)
  Q-learning resumed on B, seed 1: shortest walk again after 27 episodes (278 steps)
  Q-learning resumed on B, seed 2: shortest walk again after 13 episodes (187 steps)
  Q-learning resumed on B, seed 3: shortest walk again after 4 episodes (115 steps)
planted shortcut: map C, the agent knows only its size and goal
  planning on the free-space prior: reached true, 14 steps, 4 collisions, promised 2, 14 decisions, 130 sweeps
  planning on what it learned: reached true, 10 steps, 0 collisions, promised 10, 10 decisions, 110 sweeps
  planning on C's true table: reached true, 10 steps, 0 collisions, promised 10, 10 decisions, 110 sweeps
refused:
  S.. -> goal
  ..G -> start
  S#G -> unreachable
  S.x.G -> shape
  S./G -> shape
  S.......G -> capacity
  S/./././././././G -> capacity
  discount 1 -> discount
  discount 0.9999 with a closed-off cell -> budget
```

The [example](../examples/grid-world.coax) runs in about 5 seconds in debug
mode, compilation included, and prints the same bytes in release mode.

## The world

A map is written as rows of text separated by `/`: `.` is an open cell, `#`
a wall, `S` the start and `G` the goal. A cell's number is its row times 8
plus its column, whatever the map's width, so the map `S#G/...` has cells 0,
1 (a wall) and 2 (the goal) in its first row and 8, 9 and 10 in its second.

There are four actions: 0 up, 1 right, 2 down, 3 left. A move into a wall or
off the edge leaves the agent where it was; we count that as a collision.
Every move costs 1, and reaching the goal ends the episode. So the value of a
cell is the negative discounted number of moves to the goal: from a cell `d`
moves away, at discount γ, it is −(1 + γ + … + γ^(d−1)).

<!-- source: psionic/lib/gridworld.coax move -->
```jai
gw_move :: (m:GwMap,s:int,a:int)->int {
    // Up, right, down, left.
    dr:=[4]int.[-1,0,1,0];dc:=[4]int.[0,1,0,-1];
    row:=s/GW_SIDE+dr[a];col:=s%GW_SIDE+dc[a];
    if row<0 || col<0 || row>=m.height || col>=m.width {return s;}
    t:=row*GW_SIDE+col;
    if m.wall[t] {return s;}
    return t;
}
```

Parsing refuses a map before anything plans on it: a character other than
the four, rows of different lengths, an empty row, more than 8 rows or
columns, no goal or two, no start or two, and a goal the start cannot reach
by breadth-first search. A refused map carries no width, no walls and no
cells, so nothing downstream can mistake it for a usable one.

The planner and the agent read a transition table, `GwTable`: which cells
can be occupied, and for each cell and action, where the move leads.
`gw_truth` builds the table from the map. `gw_prior` builds what an agent
knows before it has moved: the map's size and its goal, and no walls.

## Plan exactly

Value iteration starts from zero everywhere and replaces each cell's value
with its best one-step backup, −1 + γ V(next), until the largest change in a
sweep, the Bellman residual, is at most the tolerance. The goal stays at 0.
Each sweep reads only the previous sweep's values. The plan's action in each
cell is the greedy one under the final values; on a tie, the lowest action
number wins.

<!-- source: psionic/lib/gridworld.coax plan -->
```jai
gw_backup :: (next:[]int,gamma:f64,value:[]f64,s:int,a:int)->f64 {
    return -1+gamma*value[next[4*s+a]];
}
// The action with the best backup; the lowest action wins a tie.
gw_argmax :: (next:[]int,gamma:f64,value:[]f64,s:int)->int {
    best:=0;
    for a:1..<4 {if gw_backup(next,gamma,value,s,a)>gw_backup(next,gamma,value,s,best) {best=a;}}
    return best;
}
gw_plan :: (t:GwTable,gamma:f64,tol:f64,sweeps:int)->GwPlan #export_c {
    if !(gamma>0 && gamma<1) {return GwPlan.{status=GwStatus.discount};}
    if !(tol>0) || sweeps<1 || sweeps>GW_SWEEPS_MAX || !gw_table_ok(t) {return GwPlan.{status=GwStatus.shape};}
    p:GwPlan;p.status=GwStatus.budget;next:=t.next;
    for k:0..<sweeps {
        old:=p.value;p.residual=0;p.changed=-1;p.sweeps=k+1;
        for s:0..<GW_CELLS {
            if !t.open[s] || s==t.goal {continue;}
            p.value[s]=gw_backup(next[..],gamma,old[..],s,gw_argmax(next[..],gamma,old[..],s));
            change:=abs(p.value[s]-old[s]);
            if change>p.residual {p.residual=change;p.changed=s;}
        }
        if p.residual<=tol {p.status=GwStatus.ok;break;}
    }
    for s:0..<GW_CELLS {p.action[s]=ifx t.open[s] && s!=t.goal then gw_argmax(next[..],gamma,p.value[..],s) else -1;}
    return p;
}
```

### Work the hand map

Take `S#G/...` at discount 0.5. The goal's only open neighbour is cell 10,
below it. Cell 10 is 1 move from the goal, cell 9 is 2, cell 8 is 3 and the
start is 4. Starting from zero:

| Sweep | S (0) | 8 | 9 | 10 | Residual |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | −1 | −1 | −1 | −1 | 1 |
| 2 | −1 − 0.5 = −1.5 | −1.5 | −1.5 | −1 | 0.5 |
| 3 | −1 + 0.5 × (−1.5) = −1.75 | −1.75 | −1.5 | −1 | 0.25 |
| 4 | −1 + 0.5 × (−1.75) = −1.875 | −1.75 | −1.5 | −1 | 0.125 |
| 5 | −1.875 | −1.75 | −1.5 | −1 | 0 |

In sweep 2 only cell 10 sees the goal's 0; everyone else sees a neighbour's
−1. Each later sweep carries the exact value one more move from the goal.
These are binary fractions, so the program prints exactly these numbers.
The closed form agrees: from 4 moves away, −(1 + 0.5 + 0.25 + 0.125) =
−1.875. The greedy actions are down from the start, right along the bottom
row, and up into the goal, as the example draws them.

How close is a plan that stops early? For a discounted problem, the distance
of the values from the true ones is at most γ / (1 − γ) times the last
residual. After sweep 3 above, that bound is 1 × 0.25; the true error is
0.125, in cell 0. After sweep 4 the values are already exact, but the
residual of 0.125 cannot show it until sweep 5 changes nothing. A check stops value iteration after 1 to 11 sweeps on 16
random 5 × 5 maps at discount 0.9 and confirms the bound, 9 times the
residual, in every open cell.

## Check the planner two other ways

Exhaustive search makes no use of Bellman backups. On the hand map, the four
open cells other than the goal give 4^4 = 256 deterministic policies. The
oracle follows each policy from each cell: reaching the goal in `t` moves is
worth −(1 + γ + … + γ^(t−1)), a cycle is worth −1 / (1 − γ). The best of the
256 in each cell equals the planner's value exactly. Twelve random 2 × 3 maps
at discount 0.75 repeat the comparison, and the planner's greedy policy is
optimal from every cell at once.

Shortest paths give the second check. The oracle relaxes integer distances
to the goal 64 times with its own neighbour rule, then sums the discounted
series term by term. On 16 random 5 × 5 maps, every open cell's value agrees
within 9 × 10^−12, and the plan's greedy walk from every cell takes exactly
the shortest number of moves, or never arrives where there is no route.

The oracles are in
[psionic/tests/gridworld-oracle.coax](../tests/gridworld-oracle.coax). They
share no code with the planner.

## Learn the same table from episodes

The Q-learner sees only the transitions it takes. Each episode starts in an
open cell drawn uniformly, an *exploring start*. In each step it explores
with probability `explore`/16, taking a uniformly drawn action, and otherwise
takes its greedy action, the first largest Q. Then it pulls Q(s, a) toward
−1 plus the discounted best Q of the cell it reached, or −1 alone at the
goal, and writes the transition into its model. Every random number is the
word at a fixed counter of
[chapter 53](ch053-choosing-the-next-token.md)'s counter generator, taken
modulo the number of choices: one counter for the start, two for every step.
So an episode replays from its seed and map, and a digest of every (cell,
action, result) triple shows that it does. Exploration is counted in
sixteenths so that its probability is exact; the start cell's modulo has a
bias below 2^−57, far under anything these runs can see.

<!-- source: psionic/lib/gridworld.coax learn -->
```jai
gw_learn :: (world:GwMap,q:[256]f64,model:GwTable,c:GwSettings)->GwLearn #export_c {
    if world.status!=GwStatus.ok {return GwLearn.{status=world.status};}
    if !(c.alpha>0 && c.alpha<=1) || c.explore<0 || c.explore>16 || !(c.tol>0) || c.episodes<1 || c.episodes>GW_EPISODES_MAX
        || c.episode_steps<1 || c.episode_steps>GW_EPISODE_STEPS_MAX || !gw_table_ok(model) || model.goal!=world.goal {return GwLearn.{status=GwStatus.shape};}
    // The planner's answer is read only to measure the table, never to fill it.
    truth:=gw_truth(world);plan:=gw_plan(truth,c.gamma,GW_TOL,GW_SWEEPS_MAX);
    if plan.status!=GwStatus.ok {return GwLearn.{status=plan.status};}
    fewest:=gw_predict(truth,plan,world.start);world_next:=truth.next;
    cells:[64]int;n:=0;
    for s:0..<GW_CELLS {if truth.open[s] && s!=world.goal {cells[n]=s;n+=1;}}
    l:GwLearn;l.q=q;l.model=model;l.digest=(cast(u64)14695981039346656037);counter:u64=0;
    for e:0..<c.episodes {
        // Exploring starts: every episode begins in a uniformly drawn open cell.
        s:=cells[cast(int)(cr_bits(c.seed,counter)%cast(u64)n)];counter+=1;
        for k:0..<c.episode_steps {
            if s==world.goal {break;}
            // Explore with probability explore/16: a uniformly drawn action.
            a:=gw_greedy(l.q[..],s);
            if cr_bits(c.seed,counter)%16<cast(u64)c.explore {a=cast(int)(cr_bits(c.seed,counter+1)%4);}
            counter+=2;
            // Move, write the transition into the model, and pull Q(s, a) toward
            // -1 plus the discounted best Q where the move led (zero at the goal).
            t:=world_next[4*s+a];l.model.next[4*s+a]=t;
            target:=-1.0;if t!=world.goal {target=-1+c.gamma*l.q[4*t+gw_greedy(l.q[..],t)];}
            l.q[4*s+a]+=c.alpha*(target-l.q[4*s+a]);l.steps+=1;
            l.digest=mul_wrap(l.digest^cast(u64)(4*s+a),(cast(u64)1099511628211))^cast(u64)t;
            s=t;
        }
        l.episodes=e+1;l.off=gw_q_off(truth,c.gamma,plan,l.q[..],c.tol);
        if l.close_episode==-1 && l.off==0 {l.close_episode=l.episodes;l.close_steps=l.steps;}
        if l.optimal_episode==-1 && gw_greedy_steps(world_next[..],l.q[..],world.start,world.goal)==fewest {l.optimal_episode=l.episodes;l.optimal_steps=l.steps;}
    }
    return l;
}
```

The world is deterministic, so a step size α of 1 is the right choice: each
update writes the exact one-step backup of the current table. Q-learning then
behaves like value iteration done one visited pair at a time, and it can
reach the planner's table exactly. With α = 0.5 it approaches the table
without reaching it: a check on the hand map finds pairs still more than
10^−9 off after 200 short episodes, and none more than 0.01 off.

The learner never reads the planner. `gw_learn` computes the planner's
table only to measure the learner after each episode: how many (open cell,
action) pairs are off the planner's Q by more than the tolerance of 10^−9,
and whether the greedy walk from the start takes the fewest moves. On map A,
a 5 × 4 map with two gaps in its second row, four seeds each bring every
pair within tolerance by episode 189 at the latest. Within 13 to 30 episodes
the greedy walk is already the shortest; the remaining pairs take longer,
because they lie off the useful routes. A check compares
three seeds' tables with the shortest-path oracle, Q(s, a) = −1 + 0.9 V(next),
within 10^−9 in every pair, and requires the learner's recorded transitions
to equal the map's.

## When a wall moves

Map B is map A with the gap at column 3 closed. A's best walk from the start
went left once, then down through that gap: 5 moves. On B the shortest walk
is 9 moves, through column 1.

<!-- source: psionic/lib/gridworld.coax navigate -->
```jai
// Plan on the model, take the first action in the world, write down where it
// led, plan again. A plan whose walk never reaches the goal ends the run.
gw_navigate :: (world:GwMap,model:GwTable,gamma:f64,limit:int)->GwRun #export_c {
    if world.status!=GwStatus.ok {return GwRun.{status=world.status};}
    if limit<1 || limit>GW_WALK_MAX || !gw_table_ok(model) || model.goal!=world.goal {return GwRun.{status=GwStatus.shape};}
    r:GwRun;r.model=model;s:=world.start;
    while s!=world.goal && r.steps<limit {
        p:=gw_plan(r.model,gamma,GW_TOL,GW_SWEEPS_MAX);
        if p.status!=GwStatus.ok {return GwRun.{status=p.status};}
        r.decisions+=1;r.sweeps+=p.sweeps;
        promise:=gw_predict(r.model,p,s);
        if r.decisions==1 {r.predicted=promise;}
        if promise==-1 {break;}
        a:=p.action[s];t:=gw_move(world,s,a);
        if t!=r.model.next[4*s+a] {r.surprises+=1;r.model.next[4*s+a]=t;}
        if t==s {r.collisions+=1;}
        r.steps+=1;s=t;
    }
    r.reached=s==world.goal;
    return r;
}
```

Three agents start on B with what they learned on A:

- **A fixed policy** follows A's greedy Q table without learning. It goes
  left, then down into the new wall, and stays there: 200 steps, 199
  collisions, never at the goal.
- **Replanning on A's learned model.** The model promises 5 moves. The second
  step hits the new wall; the agent records that down from cell 3 leads back
  to cell 3, plans again and goes round through column 1: 10 moves, one more
  than the shortest, with 1 collision. It planned 10 times, 9.6 sweeps per
  decision, each sweep a backup of 4 actions in every cell its model holds
  open.
- **Q-learning resumed on B,** from A's table and model and with new seeds,
  needs 4 to 52 episodes, 115 to 461 steps of experience, before its greedy
  walk is the shortest again.

The model-based agent recovers within one episode because it changes the
one transition it got wrong and replans at once. The Q-learner has to carry
the new cost back through its table one visit at a time.

## A planted shortcut

A plan is only as good as the model it plans on, and a planner picks the
actions its model makes look best. When the model is wrong in the agent's
favor, the planner chooses exactly those actions. Map C plants that case:

```text
S....
####.
G....
```

The agent knows only the map's size and its goal, so its prior has no
walls. Work it by hand. The prior says the goal is 2 moves straight down,
and the plan promises 2. The first step down hits the wall; the agent writes
that down and replans. Now the cheapest route in its model is right one
cell, then down 2: it goes right and hits the wall again. The same happens
under columns 1, 2 and 3. Under column 4 the move down works; then down
again and 4 moves left to the goal. That is 4 collisions, 4 moves right,
2 down and 4 left: 14 steps against a promise of 2. The shortest walk on the
true map is 10.

The measurement is the gap between the plan's promise and the walk, and the
count of transitions the model got wrong: promised 2, took 14, 4 surprises.
Walking C a second time with what it learned, the agent promises 10 and takes
10, the same as an agent planning on C's true table.

## Refusals

The example ends with seven maps and two plans that are refused, each with
its reason. The checks also refuse a discount of 0, 1, below 0 or NaN; a
tolerance of 0 or NaN; 0 or more than 4,096 sweeps; a table whose goal is
missing, closed or out of range, or whose transition leaves the open cells;
a model whose goal differs from the world's; a step size outside (0, 1]; an
exploration rate outside 0 to 16 sixteenths; and episode counts and lengths outside their
bounds. A plan that does not settle within its sweeps returns `budget`: at
discount 0.9999 a cell that can never reach the goal converges too slowly.
A model in which the goal has no route stops the agent before it moves.

## Supported profile

`cpu-f64-grid-v1`:

- maps of 1 to 8 rows and 1 to 8 columns, one start and one goal, the goal
  reachable from the start;
- four moves, a cost of 1 per move, the goal absorbing;
- a discount strictly between 0 and 1 and a positive tolerance; at most
  4,096 sweeps per plan, 4,096 steps per walk;
- Q-learning with α in (0, 1], exploration of 0 to 16 sixteenths, 1 to
  4,096 episodes of 1 to 256 steps, exploring starts, and chapter 53's
  SplitMix64 counter generator;
- binary64 arithmetic; the same transcript in debug and release.

## How it is checked

- **Hand values.** The sweep-by-sweep table above, the greedy actions on the
  hand map, the 14-step walk on map C and the 10-step recovery on map B are
  checks with exact expected values.
- **Exhaustive search.** All 256 policies of the hand map and every policy of
  12 random 2 × 3 maps against the planner.
- **Shortest paths.** 16 random 5 × 5 maps: values, greedy walk lengths, and
  the γ / (1 − γ) residual bound at every early stop.
- **Learning, step for step.** The oracle file holds a second Q-learner,
  written from the description above with its own neighbour rule and greedy
  choice. Six runs, on the hand map and maps A and B, with α of 1 and 0.5,
  exploration of 0, 4, 8 and 16 sixteenths and a resumed nonzero table,
  must give bit-identical Q tables, digests, step counts, learned
  transitions and episode marks.
- **Learning, converged.** Three seeds on map A against the shortest-path
  oracle within 10^−9, the recorded transitions against the map,
  bit-identical replays, and a different seed giving a different digest.
- **Refusals** at each bound named above.
- **The example** compares its transcript exactly in debug and release.
- **Mutation testing** of `gridworld.coax` with `--root-only` kills all 527 mutants. The first run left 71 survivors, mostly where a check could not tell two programs apart: a maximum that keeps the first or the last of equal values, a loop bound past the longest possible walk, and exploration at exactly one half, where testing a draw below the rate and above it are the same. The planner now reports which cell changed most, the lowest on a tie; walks stop when they revisit a cell instead of at a step cap; exploration is counted in sixteenths; and a second Q-learner in the oracle file pins every draw, update and mark.

## What this does not do yet

- **Behavior cloning and DAgger.** A small policy trained with chapter 103's
  trainer on the planner's demonstrations, then on planner labels for the
  states it visits, compared at the same number of labels. Not built.
- **Planning with chapter 109's learned model.** The learned model here is a
  table of observed transitions with a free-space prior, not the continuous
  surrogate of chapter 109. The planted case shows the measurement on a
  table; the continuous version is not built.
- **Held-out maps.** Results are on three fixed maps and their seeds, not on
  a suite of held-out maps and parameters with completion rates.
- **Deterministic moves only.** No slipping, no noise in observations.
- **A Verse character** that navigates and recovers needs its own consumer
  contract.
