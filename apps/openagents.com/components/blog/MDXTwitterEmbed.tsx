'use client'

import React, { useEffect } from 'react'

export function MDXTwitterEmbed({ html }: { html: string }) {
  useEffect(() => {
    // @ts-ignore
    if (window.twttr) {
      // @ts-ignore
      window.twttr.widgets.load()
    } else {
      const script = document.createElement('script')
      script.src = 'https://platform.twitter.com/widgets.js'
      script.async = true
      document.body.appendChild(script)
    }
  }, [])

  return (
    <div 
      className="w-full mx-auto flex justify-center items-center my-8"
      dangerouslySetInnerHTML={{ __html: html }}
    />
  )
}