'use client'

import { useEffect } from 'react'

export function TwitterEmbed({ tweetId }: { tweetId: string }) {
  useEffect(() => {
    // Load Twitter widget script if it hasn't been loaded yet
    if (!(window as any).twttr) {
      const script = document.createElement('script')
      script.src = 'https://platform.twitter.com/widgets.js'
      script.async = true
      script.charset = 'utf-8'
      document.body.appendChild(script)
    } else {
      // If already loaded, refresh widgets
      ;(window as any).twttr.widgets.load()
    }
  }, [])

  return (
    <div className="w-full mx-auto flex justify-center items-center my-8">
      <blockquote className="twitter-tweet" data-media-max-width="560">
        <a href={`https://twitter.com/x/status/${tweetId}`}>Loading tweet...</a>
      </blockquote>
    </div>
  )
}