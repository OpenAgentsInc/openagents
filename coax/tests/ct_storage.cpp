// Exercise the actual evaluator's aggregate owner and allocator under sanitizers.
#define main coax_driver_main
#include "../src/main.cpp"
#undef main
#include <cassert>

int main() {
  using namespace coax;
  File file;
  Evaluator evaluator{&file};
  auto memory = file.ct_memory;
  {
    auto owner = evaluator.storage();
    owner->reserve(20);
    assert(memory->slots == owner->capacity() && memory->containers == 1);
    auto view = owner;
    owner.reset();
    assert(memory->slots == view->capacity() && memory->containers == 1);
    view.reset();
    assert(memory->slots == 0 && memory->containers == 0);
  }
  {
    auto full = evaluator.storage();
    full->reserve(CtMemory::max_slots());
    assert(memory->slots == CtMemory::max_slots());
    auto another = evaluator.storage();
    try { another->reserve(1); assert(false); } catch (const CtLimit &) {}
    assert(memory->slots == CtMemory::max_slots() && another->empty());
  }
  assert(memory->slots == 0 && memory->containers == 0);
  {
    std::vector<std::shared_ptr<CtStorage>> all;
    all.reserve(CtMemory::max_containers());
    for (uint64_t i = 0; i < CtMemory::max_containers(); ++i) all.push_back(evaluator.storage());
    try { auto extra = evaluator.storage(); (void)extra; assert(false); } catch (const CtLimit &) {}
    assert(memory->containers == CtMemory::max_containers());
    all.pop_back();
    auto extra = evaluator.storage();
    assert(memory->containers == CtMemory::max_containers());
  }
  assert(memory->slots == 0 && memory->containers == 0);
  for (uint64_t i = 0; i < CtMemory::max_containers() * 3; ++i) {
    auto ephemeral = evaluator.storage();
  }
  assert(memory->stores.size() <= CtMemory::max_containers() * 2);
  assert(memory->slots == 0 && memory->containers == 0);
  // Nested aggregate clone must charge every backing vector, and failure while
  // cloning unwinds its partially constructed graph instead of leaking slots.
  TypeInfo arr; arr.kind = TypeKind::array;
  Value outer; outer.type = &arr; outer.agg = evaluator.storage();
  Value inner; inner.type = &arr; inner.agg = evaluator.storage();
  inner.agg->resize(8);
  outer.agg->push_back(inner);
  const auto before = memory->slots;
  {
    auto copy = evaluator.clone(outer);
    assert(memory->slots > before && copy.agg != outer.agg);
    assert((*copy.agg)[0].agg != inner.agg);
  }
  assert(memory->slots == before);
  {
    auto filler = evaluator.storage();
    filler->reserve(CtMemory::max_slots() - memory->slots - 1);
    const auto occupied = memory->slots;
    try { auto copy = evaluator.clone(outer); (void)copy; assert(false); } catch (const CtLimit &) {}
    assert(memory->slots == occupied);
  }
  {
    std::shared_ptr<CtMemory> retained;
    std::weak_ptr<CtStorage> weak;
    {
      File cyclic;
      Evaluator e{&cyclic};
      retained = cyclic.ct_memory;
      auto storage = e.storage();
      weak = storage;
      Value view; view.type = &arr; view.agg = storage;
      storage->push_back(view);
      storage.reset();
      assert(!weak.expired());
    }
    assert(weak.expired());
    assert(retained->slots == 0 && retained->containers == 0);
  }
  return 0;
}
