import React from 'react'
import { animated as a, useTrail } from '@react-spring/web'

export const MessagesList = ({ messages }) => {
  return (
    <Trail open={true}>
      {messages.map((message, i) => (
        <span key={i}>{message.content}</span>
      ))}
    </Trail>
  )
}

const Trail: React.FC<{ open: boolean, children: any }> = ({ open, children }) => {
  const items = React.Children.toArray(children)
  const trail = useTrail(items.length, {
    config: { mass: 5, tension: 2000, friction: 200 },
    opacity: open ? 1 : 0,
    x: open ? 0 : 20,
    from: { opacity: 0, x: 20 }
  })
  return (
    <div>
      {trail.map((style, index) => (
        <a.div key={index} style={style} className="whitespace-pre-wrap mb-8 last:mb-0">
          <a.div>{items[index]}</a.div>
        </a.div>
      ))}
    </div>
  )
}
