import React from 'react'
import { animated as a, useTrail } from '@react-spring/web'

const Message = ({ message }) => {
  return (
    <span>{message.content}</span>
  )
}

export const MessagesList = ({ messages }) => {
  return (
    <div className="w-full">
      <Trail open={true}>
        {messages.map((message, i) => (<Message key={i} message={message} />))}
      </Trail>
    </div>
  )
}

const Trail: React.FC<{ open: boolean, children: any }> = ({ open, children }) => {
  const items = React.Children.toArray(children)
  const trail = useTrail(items.length, {
    config: { mass: 5, tension: 2000, friction: 200 },
    opacity: open ? 1 : 0,
    y: open ? 0 : 20,
    from: { opacity: 0, y: 20 }
  })
  return (
    <div>
      {trail.map((style, index) => {
        // @ts-ignore
        const role = items[index].props.message.role
        if (role === "user") {
          return (
            <a.div key={index} style={style}>
              <a.div className="flex justify-end break-anywhere relative py-1">
                <div className="max-w-[83%] space-y-1 whitespace-pre-wrap">
                  <div className="rounded-[10px] bg-neutral-200 p-3 ml-auto w-fit max-w-full">
                    {items[index]}
                  </div>
                </div>
              </a.div>
            </a.div>
          )
        }
        return (
          <a.div key={index} style={style}>
            <a.div className="whitespace-pre-wrap mb-4">{items[index]}</a.div>
          </a.div>
        )
      })}
    </div>
  )
}
