import * as React from "react"
import { useDropzone } from "react-dropzone"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import {
  Card, CardContent, CardDescription, CardHeader, CardTitle
} from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { useForm, usePage } from "@inertiajs/react"
import { RocketIcon } from "@radix-ui/react-icons"

export function UploadDocForm() {
  const onDrop = React.useCallback((acceptedFiles) => {
    setData('file', acceptedFiles[0])
  }, [])

  const { getRootProps, getInputProps, isDragActive } = useDropzone({ onDrop })

  const props = usePage().props
  // console.log(props)
  const { data, setData, post, progress } = useForm({
    file: null
  })

  React.useEffect(() => {
    if (!data.file) return
    // Make API request
    post('/api/files', {
      onSuccess: (res) => {
        console.log(res)
        setData('file', null)
      },
      onError: (err) => {
        console.log(err)
      }
    })
  }, [data.file])

  const filename = 'asodfujass'
  // const filename = props.flash?.filename ?? null

  return (
    <Card className="w-[400px]">
      <CardHeader>
        <CardTitle>Upload a PDF</CardTitle>
        <CardDescription>Teach agents about your project</CardDescription>
      </CardHeader>

      <CardContent>
        {!!props.progress && (
          <Alert className="mb-4">
            <RocketIcon className="h-4 w-4" />
            <AlertTitle>Uploading</AlertTitle>
            <AlertDescription>{props.progress}</AlertDescription>
          </Alert>
        )}
        {/* {props.flash?.message && (
          <Alert className="mb-4">
            <RocketIcon className="h-4 w-4" />
            <AlertTitle>{props.flash.message === 'File uploaded.' ? 'Success!' : 'Message'}</AlertTitle>
            <AlertDescription>{props.flash.message}</AlertDescription>
          </Alert>
        )} */}
        <div {...getRootProps()} className="grid w-full items-center gap-4">
          <label
            className="border-2 border-dashed border-border rounded-lg p-6 text-center h-64 flex flex-col justify-center items-center cursor-pointer"
            htmlFor="file"
          >
            <svg
              className="text-gray-400 mx-auto mb-4 w-12 h-12"
              fill="none"
              height="24"
              stroke="currentColor"
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth="2"
              viewBox="0 0 24 24"
              width="24"
              xmlns="http://www.w3.org/2000/svg"
            >
              <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
              <polyline points="17 8 12 3 7 8" />
              <line x1="12" x2="12" y1="3" y2="15" />
            </svg>
            <span>{isDragActive ? <p>Feed me</p> : <p>Drop a PDF here</p>}</span>
            <Input className="sr-only" id="file" type="file" {...getInputProps()} />
          </label>
          <div className="flex flex-col space-y-1.5 hidden">
            <Label htmlFor="progress">Upload Progress</Label>
            <div className="h-3 bg-gray-700 rounded" id="progress">
              <div
                className="h-full bg-green-500 rounded"
                style={{
                  width: '0%'
                }}
              />
            </div>
          </div>
        </div>
      </CardContent>

    </Card>
  )
}
