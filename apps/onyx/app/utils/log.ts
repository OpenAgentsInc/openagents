interface LogProps {
  name: string
  preview?: string
  value?: any
  important?: boolean
}

function createLog(input: LogProps | string) {
  if (typeof input === 'string') {
    if (__DEV__) {
      console.tron?.display({ name: input })
    } else {
      console.log(input)
    }
  } else {
    if (__DEV__) {
      console.tron?.display(input)
      console.log(input)
    } else {
      console.log(input)
    }
  }
}

export const log = Object.assign(createLog, {
  error: (name: string, error?: any) => {
    createLog({
      name,
      preview: "Error",
      value: error,
      important: true,
    })
  }
})
