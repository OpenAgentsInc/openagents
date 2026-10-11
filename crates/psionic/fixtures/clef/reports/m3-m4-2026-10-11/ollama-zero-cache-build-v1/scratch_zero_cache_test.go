package mlxrunner

import (
	"os"
	"path/filepath"
	"slices"
	"testing"

	"github.com/ollama/ollama/mlx"
	"github.com/ollama/ollama/mlxrunner/cache"
)

// These tests use upstream scalar fakes. Every State method returns no arrays,
// so cacheSession.close's AsyncEval returns before entering the native evaluator.
type scratchHidden struct {
	fakeRewindableCache
	frees int
}

func (h *scratchHidden) Free() {
	for _, snapshot := range h.TakeSnapshots() {
		if snapshot != nil {
			snapshot.Close()
		}
	}
	h.fakeRewindableCache.Free()
	h.frees++
}

type scratchEnv struct {
	pc      *prefixCache
	tracker *snapshotTracker
	kv      *fakeRewindableCache
	delta   *fakeRecurrentCache
	hidden  *scratchHidden
}

func scratchReset(t *testing.T) bool {
	t.Helper()
	switch os.Getenv("CLEF_SCRATCH_EXPECT_RESET") {
	case "stock":
		return false
	case "reset":
		return true
	default:
		t.Fatal("explicit stock or reset expectation is required")
		return false
	}
}

func newScratchEnv(t *testing.T) *scratchEnv {
	t.Helper()
	tr := &snapshotTracker{}
	e := &scratchEnv{tracker: tr, kv: &fakeRewindableCache{tracker: tr},
		delta:  &fakeRecurrentCache{tracker: tr},
		hidden: &scratchHidden{fakeRewindableCache: fakeRewindableCache{tracker: tr}}}
	e.pc = &prefixCache{caches: []cache.Cache{e.kv, e.delta, nil, e.hidden}}
	t.Cleanup(func() {
		e.pc.close()
		checkSnapshotLeaks(t, tr, e.pc.root)
	})
	return e
}

func (e *scratchEnv) tokens(t *testing.T, want []int32) {
	t.Helper()
	for name, got := range map[string][]int32{"kv": e.kv.tokens, "recurrent": e.delta.tokens, "hidden": e.hidden.tokens} {
		if !slices.Equal(got, want) {
			t.Fatalf("%s token state differs: length %d, want %d", name, len(got), len(want))
		}
	}
	assertCacheOffsetAlignment(t, e.pc, "scalar state")
}

// stopAt can end a request early. The cancellation case deliberately relies on
// session.close to attach and drain captures, just like the production defer.
func (e *scratchEnv) score(t *testing.T, inputs []int32, stopAt int, cancel bool) int {
	t.Helper()
	reset := scratchReset(t)
	old := slices.Clone(e.tracker.all)
	frees := e.hidden.frees
	session := e.pc.beginScore(inputs, nil)
	defer session.close()
	offset := len(inputs) - len(session.remaining)
	if reset {
		if offset != 0 || !slices.Equal(session.remaining, inputs) || e.hidden.frees <= frees {
			t.Fatal("reset score did not start with all input and cleared hidden state")
		}
		e.tokens(t, nil)
		if e.pc.pagedOutBytes != 0 || e.pc.root == nil || len(e.pc.root.children) != 0 {
			t.Fatal("reset score retained old trie state")
		}
		for _, snapshot := range old {
			if snapshot.closeCount != 1 {
				t.Fatal("old snapshot was not closed exactly once before the next score")
			}
		}
	}
	if offset < 0 || offset > stopAt || stopAt > len(inputs) {
		t.Fatal("invalid scalar test offset")
	}
	e.tokens(t, inputs[:offset])
	var snapshots []int
	for at := 7; at < stopAt; at += 97 {
		if at > offset {
			snapshots = append(snapshots, at)
		}
	}
	session.schedulePrefillSnapshots(snapshots)
	for at := offset; at < stopAt; {
		end := min(at+17, stopAt)
		feedAll(e.pc.caches, inputs[at:end])
		e.tokens(t, inputs[:end])
		at = end
	}
	if !cancel {
		session.attachPrefillSnapshots()
	}
	return offset
}

func scratchInput(n int) []int32 {
	ids := make([]int32, n)
	for i := range ids {
		ids[i] = int32(100 + i)
	}
	return ids
}

func TestScratchZeroCacheFirstAndRepeat(t *testing.T) {
	e, ids := newScratchEnv(t), scratchInput(41)
	if e.score(t, ids, len(ids), false) != 0 {
		t.Fatal("first score reused tokens")
	}
	got := e.score(t, ids, len(ids), false)
	if scratchReset(t) && got != 0 || !scratchReset(t) && got != len(ids) {
		t.Fatalf("repeat offset %d does not match the build's declared mode", got)
	}
	checkSnapshotLeaks(t, e.tracker, e.pc.root)
}

func TestScratchZeroCacheDivergent(t *testing.T) {
	e, a := newScratchEnv(t), scratchInput(47)
	e.score(t, a, len(a), false)
	b := slices.Clone(a)
	for i := 23; i < len(b); i++ {
		b[i] += 1000
	}
	e.score(t, b, len(b), false)
	e.score(t, a, len(a), false)
	checkTrieInvariants(t, e.pc.root)
	checkSnapshotLeaks(t, e.tracker, e.pc.root)
}

func TestScratchZeroCacheLongChunks(t *testing.T) {
	e, ids := newScratchEnv(t), scratchInput(1057)
	e.score(t, ids, len(ids), false)
	if len(e.tracker.all) < 10 {
		t.Fatal("long case did not create enough snapshots to exercise cleanup")
	}
	e.score(t, ids, len(ids), false)
	e.tokens(t, ids)
	checkSnapshotLeaks(t, e.tracker, e.pc.root)
}

func TestScratchZeroCacheCancelThenFresh(t *testing.T) {
	e, ids := newScratchEnv(t), scratchInput(211)
	e.score(t, ids, 113, true)
	checkSnapshotLeaks(t, e.tracker, e.pc.root)
	got := e.score(t, ids, len(ids), false)
	if !scratchReset(t) && got == 0 {
		t.Fatal("stock control did not retain canceled prefill state")
	}
	e.tokens(t, ids)
	checkSnapshotLeaks(t, e.tracker, e.pc.root)
}

func TestScratchZeroCacheRepeatedClose(t *testing.T) {
	e, ids := newScratchEnv(t), scratchInput(61)
	e.score(t, ids, len(ids), false)
	e.pc.close()
	e.pc.close()
	e.tokens(t, nil)
	if e.pc.root != nil || len(e.pc.activePath) != 0 || e.pc.pagedOutBytes != 0 {
		t.Fatal("repeated close retained trie state")
	}
	checkSnapshotLeaks(t, e.tracker, nil)
	if e.score(t, ids, len(ids), false) != 0 {
		t.Fatal("score after explicit close reused state")
	}
}

func TestScratchGenerationUnchanged(t *testing.T) {
	_ = scratchReset(t)
	e, ids := newScratchEnv(t), scratchInput(31)
	simulateRequest(t, e.pc, ids, nil, 7)
	result := simulateRequest(t, e.pc, ids, nil, 7)
	if len(result.remaining) != 1 {
		t.Fatalf("generation changed: remaining %d, want the one-token seed", len(result.remaining))
	}
	checkSnapshotLeaks(t, e.tracker, e.pc.root)
}

func TestScratchLibraryIdentity(t *testing.T) {
	_ = scratchReset(t)
	if err := mlx.CheckInit(); err != nil {
		t.Fatal(err)
	}
	got, err := mlx.LoadedLibraryPath()
	if err != nil {
		t.Fatal(err)
	}
	got, err = filepath.EvalSymlinks(got)
	if err != nil || got != os.Getenv("CLEF_SCRATCH_EXPECT_LIBRARY") {
		t.Fatalf("unexpected native library: %q, error %v", got, err)
	}
	if version := mlx.Version(); version != "0.32.3-53-ga59cc23" {
		t.Fatalf("unexpected MLX version %q", version)
	}
	t.Logf("native library %s, MLX %s; no arrays or model created", got, mlx.Version())
}
