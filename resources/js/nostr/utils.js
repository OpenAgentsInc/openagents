// some basic checks for plugin events
export const checkPluginEvent = (event) => {
  const wasm_url = event.content;
  const slug = event.tags.find((e) => e[0] === 'd')?.[1];
  const title = event.tags.find((e) => e[0] === 'title')?.[1];
  const description = event.tags.find((e) => e[0] === 'summary')?.[1];
  const fee = Number(event.tags.find((e) => e[0] === 'price')?.[1] || 0);
  const feeCurrency = event.tags.find((e) => e[0] === 'price')?.[2] || '';
  const published = event.tags.find((e) => e[0] === 'published_at')?.[1];

  if (fee !== 0) {
    if (feeCurrency !== 'btc') {
      return false
    }
  }

  if (!published || slug.length < 2 || title.length > 512 || title.length < 2 || description < 3 || description > 5012 || wasm_url > 512 || wasm_url < 4) {
    return false
  }
  const regex = /((([A-Za-z]{3,9}:(?:\/\/)?)(?:[\-;:&=\+\$,\w]+@)?[A-Za-z0-9\.\-]+|(?:www\.|[\-;:&=\+\$,\w]+@)[A-Za-z0-9\.\-]+)((?:\/[\+~%\/\.\w\-_]*)?\??(?:[\-\+=&;%@\.\w_]*)#?(?:[\.\!\/\\\w]*))?)/;
  if (regex.test(wasm_url) === false) {
    return false
  }

  return {slug, id: event.id, author: event.pubkey, wasm_url, title, description, published: Number(published), fee}
}

// Format sats numbers
export const formatSats = (number) => {
  if (number >= 1000 && number < 10000) {
    return Math.round((number / 100) * 10) / 10 + 'k';
  } else if (number >= 10000 && number < 1000000) {
    return Math.round((number / 10000) * 10) / 10 + 'k';
  } else {
    return number;
  }
}

