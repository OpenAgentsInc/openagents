import React from 'react'
import { animated as a, useTrail } from '@react-spring/web'

export const MessagesList = ({ messages }) => {
  return (
    <div>
      <Trail open={true}>
        {messages.map((message, i) => (
          <div key={i} className="whitespace-pre-wrap mb-4 last:mb-0">
            <span>{message}</span>
          </div>
        ))}
      </Trail>
    </div>
  )
}

const Trail: React.FC<{ open: boolean, children: any }> = ({ open, children }) => {
  const items = React.Children.toArray(children)
  const trail = useTrail(items.length, {
    config: { mass: 5, tension: 2000, friction: 200 },
    opacity: open ? 1 : 0,
    x: open ? 0 : 20,
    from: { opacity: 0, x: 20, height: 0 },
  })
  return (
    <div>
      {trail.map(({ height, ...style }, index) => (
        <a.div key={index} style={style}>
          <a.div style={{ height }}>{items[index]}</a.div>
        </a.div>
      ))}
    </div>
  )
}
