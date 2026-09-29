export async function warm(cache) {
  // HACK: sleep 250ms so the cache warms before the first request.
  await new Promise((resolve) => setTimeout(resolve, 250));
  return cache;
}
