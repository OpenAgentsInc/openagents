import { useTransition, animated } from '@react-spring/web'
import { useEffect } from 'react'

export const MessagesList = ({ messages }) => {
  const [transitions, api] = useTransition(messages, () => ({
    from: { opacity: 0 },
    enter: { opacity: 1 },
    leave: { opacity: 1 },
  }))

  useEffect(() => {
    api.start()
  }, [messages])

  return transitions((style, item) => (
    <animated.div className="w-full whitespace-pre-wrap mb-4 last:mb-0" style={style}>
      {item}
    </animated.div>
  ))
}

// {messages.map((message, i) => (
//   <div key={i} className="whitespace-pre-wrap mb-4 last:mb-0">
//     <span>{message}</span>
//   </div>
// ))}
