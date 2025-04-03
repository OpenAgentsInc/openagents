import React, { useEffect, useRef, useState, useCallback, useMemo } from "react"
import { AnimatePresence, motion } from "framer-motion"
import { ArrowUp, Info, Loader2, Mic, Paperclip, Square, X } from "lucide-react"
import { omit } from "remeda"

import { cn } from "@/utils/tailwind"
import { useAudioRecording } from "@/hooks/use-audio-recording"
import { useAutosizeTextArea } from "@/hooks/use-autosize-textarea"
import { useFocusInput } from "@/hooks/use-focus-input"
import { AudioVisualizer } from "@/components/ui/audio-visualizer"
import { Button } from "@/components/ui/button"
import { FilePreview } from "@/components/ui/file-preview"
import { InterruptPrompt } from "@/components/ui/interrupt-prompt"
import { ModelSelect } from '@/components/ui/model-select'

interface MessageInputBaseProps
  extends React.TextareaHTMLAttributes<HTMLTextAreaElement> {
  value: string
  submitOnEnter?: boolean
  stop?: () => void
  isGenerating: boolean
  selectedModelId: string
  handleModelChange: (value: string) => void
  isModelAvailable: boolean
  enableInterrupt?: boolean
  transcribeAudio?: (blob: Blob) => Promise<string>
}

interface MessageInputWithoutAttachmentProps extends MessageInputBaseProps {
  allowAttachments?: false
}

interface MessageInputWithAttachmentsProps extends MessageInputBaseProps {
  allowAttachments: true
  files: File[] | null
  setFiles: React.Dispatch<React.SetStateAction<File[] | null>>
}

type MessageInputProps =
  | MessageInputWithoutAttachmentProps
  | MessageInputWithAttachmentsProps

export const MessageInput = React.forwardRef<HTMLTextAreaElement, MessageInputProps>(({
  placeholder = "Ask Coder",
  className,
  onKeyDown: onKeyDownProp,
  submitOnEnter = true,
  stop,
  isGenerating,
  selectedModelId,
  handleModelChange,
  isModelAvailable,
  enableInterrupt = true,
  transcribeAudio,
  allowAttachments,
  value,
  onChange,
  ...restProps
}: MessageInputProps, ref) => {
  // Extract files and setFiles from restProps if allowAttachments is true
  const { files, setFiles, ...props } = allowAttachments === true
    ? restProps as MessageInputWithAttachmentsProps
    : { files: null, setFiles: null, ...restProps };

  const [isDragging, setIsDragging] = useState(false);
  const [showInterruptPrompt, setShowInterruptPrompt] = useState(false);

  // Store the onChange handler in a ref to avoid creating functions
  const onChangeRef = useRef(onChange);

  // Update the ref when props.onChange changes
  useEffect(() => {
    onChangeRef.current = onChange;
  }, [onChange]);

  // Memoize the speech callbacks
  const onTranscriptionComplete = useCallback((text: string) => {
    onChangeRef.current?.({ target: { value: text } } as any);
  }, []);

  const {
    isListening,
    isSpeechSupported,
    isRecording,
    isTranscribing,
    audioStream,
    toggleListening,
    stopRecording,
  } = useAudioRecording({
    transcribeAudio,
    onTranscriptionComplete
  })

  // Store the value and allowAttachments in refs to avoid recreating functions
  const valueRef = useRef(value);
  const filesRef = useRef(files);
  const allowAttachmentsRef = useRef(allowAttachments);
  const setFilesRef = useRef(setFiles);

  // Update refs when props change
  useEffect(() => {
    valueRef.current = value;
    filesRef.current = files;
    setFilesRef.current = setFiles;
    allowAttachmentsRef.current = allowAttachments;
  }, [value, files, allowAttachments, setFiles]);

  useEffect(() => {
    if (!isGenerating) {
      setShowInterruptPrompt(false)
    }
  }, [isGenerating])

  // All event handlers memoized with useCallback
  const addFiles = useCallback((files: File[] | null) => {
    if (!allowAttachmentsRef.current || !setFilesRef.current) return;

    setFilesRef.current((currentFiles) => {
      if (currentFiles === null) {
        return files;
      }

      if (files === null) {
        return currentFiles;
      }

      return [...currentFiles, ...files];
    });
  }, []);

  const onDragOver = useCallback((event: React.DragEvent) => {
    if (allowAttachmentsRef.current !== true) return
    event.preventDefault()
    setIsDragging(true)
  }, []);

  const onDragLeave = useCallback((event: React.DragEvent) => {
    if (allowAttachmentsRef.current !== true) return
    event.preventDefault()
    setIsDragging(false)
  }, []);

  const onDrop = useCallback((event: React.DragEvent) => {
    setIsDragging(false)
    if (allowAttachmentsRef.current !== true) return
    event.preventDefault()
    const dataTransfer = event.dataTransfer
    if (dataTransfer.files.length) {
      addFiles(Array.from(dataTransfer.files))
    }
  }, [addFiles]);

  const onPaste = useCallback((event: React.ClipboardEvent) => {
    const items = event.clipboardData?.items
    if (!items) return

    const text = event.clipboardData.getData("text")
    if (text && text.length > 500 && allowAttachmentsRef.current) {
      event.preventDefault()
      const blob = new Blob([text], { type: "text/plain" })
      const file = new File([blob], "Pasted text", {
        type: "text/plain",
        lastModified: Date.now(),
      })
      addFiles([file])
      return
    }

    const files = Array.from(items)
      .map((item) => item.getAsFile())
      .filter((file) => file !== null)

    if (allowAttachmentsRef.current && files.length > 0) {
      addFiles(files)
    }
  }, [addFiles]);

  const onKeyDown = useCallback((event: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (submitOnEnter && event.key === "Enter" && !event.shiftKey) {
      event.preventDefault()

      if (isGenerating && stop && enableInterrupt) {
        if (showInterruptPrompt) {
          stop()
          setShowInterruptPrompt(false)
          event.currentTarget.form?.requestSubmit()
        } else if (
          valueRef.current ||
          (allowAttachmentsRef.current && filesRef.current?.length)
        ) {
          setShowInterruptPrompt(true)
          return
        }
      }

      event.currentTarget.form?.requestSubmit()
    }

    onKeyDownProp?.(event)
  }, [submitOnEnter, isGenerating, stop, enableInterrupt, showInterruptPrompt, onKeyDownProp]);

  const textAreaRef = useRef<HTMLTextAreaElement>(null)
  const [textAreaHeight, setTextAreaHeight] = useState<number>(0)

  // Memoize this effect that measures textarea height
  useEffect(() => {
    if (textAreaRef.current) {
      setTextAreaHeight(textAreaRef.current.offsetHeight)
    }
  }, [value])

  // Use the focus hook - textAreaRef is correctly typed for this hook
  useFocusInput(textAreaRef as React.RefObject<HTMLTextAreaElement>)

  // Memoize this computation
  const showFileList = useMemo(() =>
    allowAttachments && files && files.length > 0,
    [allowAttachments, files]);

  // Auto-size the textarea
  useAutosizeTextArea({
    ref: textAreaRef,
    value: value || ''
  })

  // File removal handler - memoized to avoid recreation
  const handleFileRemove = useCallback((file: File) => {
    if (!allowAttachmentsRef.current || !setFilesRef.current) return;

    setFilesRef.current((files) => {
      if (!files) return null;

      const filtered = Array.from(files).filter(f => f !== file);
      return filtered.length === 0 ? null : filtered;
    });
  }, []);

  // Close interrupt prompt handler
  const closeInterruptPrompt = useCallback(() => {
    setShowInterruptPrompt(false);
  }, []);

  // Button click handler
  const handleAttachClick = useCallback(async () => {
    const files = await showFileUploadDialog();
    addFiles(files);
  }, [addFiles]);

  // Memoize textarea props to avoid spread recreation
  const textareaProps = useMemo(() => {
    // We can now just use the cleaned props directly
    return props;
  }, [props]);

  // Memoize rendered files
  const renderedFiles = useMemo(() => {
    if (!allowAttachments || !files) return null;

    return (
      <AnimatePresence mode="popLayout">
        {files.map((file) => (
          <FilePreview
            key={file.name + String(file.lastModified)}
            file={file}
            onRemove={() => handleFileRemove(file)}
          />
        ))}
      </AnimatePresence>
    );
  }, [allowAttachments, files, handleFileRemove]);

  return (
    <div
      className="relative flex w-full flex-col"
      onDragOver={onDragOver}
      onDragLeave={onDragLeave}
      onDrop={onDrop}
    >
      <div className="relative flex w-full flex-col items-stretch gap-2 border-x border-t bg-background px-3 sm:px-4 pt-3 sm:pt-4 pb-2">
        <div className="flex flex-grow flex-col">
          <textarea
            aria-label="Write your prompt here"
            autoFocus
            placeholder={placeholder}
            ref={textAreaRef}
            value={value}
            onChange={onChange}
            onPaste={onPaste}
            onKeyDown={onKeyDown}
            onFocus={(e) => {
              setTimeout(() => e.target.focus(), 0);
            }}
            onClick={(e) => (e.target as HTMLElement).focus()}
            className={cn(
              "mb-2 w-full resize-none bg-transparent text-base md:text-sm leading-6 outline-none disabled:opacity-0 min-h-[44px] touch-manipulation",
              showFileList && "pb-16",
              className
            )}
            {...textareaProps}
          />
          <div className="flex flex-col gap-2 md:flex-row md:items-center">
            <div className="flex flex-wrap items-center gap-2 pb-1 md:pb-0 overflow-x-auto">
              <div className={cn("w-auto min-w-[120px] flex-shrink-0", !isModelAvailable && "opacity-70")}>
                <ModelSelect value={selectedModelId} onChange={handleModelChange} />
              </div>
              {/* Comment out AgentSelection and ToolSelection for now */}
              {/*<div className={cn("w-auto min-w-[100px] flex-shrink-0", !isModelAvailable && "opacity-70")}>
                <AgentSelection />
              </div>
              <div className={cn("w-auto min-w-[80px] flex-shrink-0", !isModelAvailable && "opacity-70")}>
                <ToolSelection />
              </div>*/}
            </div>
          </div>
        </div>
      </div>

      {enableInterrupt && (
        <InterruptPrompt
          isOpen={showInterruptPrompt}
          close={closeInterruptPrompt}
        />
      )}

      <RecordingPrompt
        isVisible={isRecording}
        onStopRecording={stopRecording}
      />

      <div className="absolute right-3 top-3 z-20 flex gap-2">
        {allowAttachments && (
          <Button
            type="button"
            size="icon"
            variant="outline"
            className="h-8 w-8"
            aria-label="Attach a file"
            onClick={handleAttachClick}
          >
            <Paperclip className="h-4 w-4" />
          </Button>
        )}
        {isSpeechSupported && (
          <Button
            type="button"
            variant="outline"
            className={cn("h-8 w-8", isListening && "text-primary")}
            aria-label="Voice input"
            size="icon"
            onClick={toggleListening}
          >
            <Mic className="h-4 w-4" />
          </Button>
        )}
        {isGenerating && stop ? (
          <Button
            type="button"
            size="icon"
            className="h-8 w-8"
            aria-label="Stop generating"
            onClick={stop}
          >
            <Square className="h-3 w-3 animate-pulse" fill="currentColor" />
          </Button>
        ) : (
          <Button
            type="submit"
            size="icon"
            className="h-8 w-8 transition-opacity"
            aria-label="Send message"
            disabled={value.trim() === "" || isGenerating}
            onClick={() => {
              console.log('Submit button clicked');
              console.log('Value:', value);
              console.log('isGenerating:', isGenerating);
              console.log('Disabled:', value.trim() === "" || isGenerating);
            }}
          >
            <ArrowUp className="h-5 w-5" />
          </Button>
        )}
      </div>

      {allowAttachments && <FileUploadOverlay isDragging={isDragging} />}

      <RecordingControls
        isRecording={isRecording}
        isTranscribing={isTranscribing}
        audioStream={audioStream}
        textAreaHeight={textAreaHeight}
        onStopRecording={stopRecording}
      />
    </div>
  );
});

// Set display name
MessageInput.displayName = "MessageInput";

interface FileUploadOverlayProps {
  isDragging: boolean
}

function FileUploadOverlay({ isDragging }: FileUploadOverlayProps) {
  return (
    <AnimatePresence>
      {isDragging && (
        <motion.div
          className="pointer-events-none absolute inset-0 z-20 flex items-center justify-center space-x-2  border border-dashed border-border bg-background text-sm text-muted-foreground"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: 0.2 }}
          aria-hidden
        >
          <Paperclip className="h-4 w-4" />
          <span>Drop your files here to attach them.</span>
        </motion.div>
      )}
    </AnimatePresence>
  )
}

function showFileUploadDialog() {
  const input = document.createElement("input")

  input.type = "file"
  input.multiple = true
  input.accept = "*/*"
  input.click()

  return new Promise<File[] | null>((resolve) => {
    input.onchange = (e) => {
      const files = (e.currentTarget as HTMLInputElement).files

      if (files) {
        resolve(Array.from(files))
        return
      }

      resolve(null)
    }
  })
}

function TranscribingOverlay() {
  return (
    <motion.div
      className="flex h-full w-full flex-col items-center justify-center bg-background/80 backdrop-blur-sm"
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      transition={{ duration: 0.2 }}
    >
      <div className="relative">
        <Loader2 className="h-8 w-8 animate-spin text-primary" />
        <motion.div
          className="absolute inset-0 h-8 w-8 animate-pulse rounded-full bg-primary/20"
          initial={{ scale: 0.8, opacity: 0 }}
          animate={{ scale: 1.2, opacity: 1 }}
          transition={{
            duration: 1,
            repeat: Infinity,
            repeatType: "reverse",
            ease: "easeInOut",
          }}
        />
      </div>
      <p className="mt-4 text-sm font-medium text-muted-foreground">
        Transcribing audio...
      </p>
    </motion.div>
  )
}

interface RecordingPromptProps {
  isVisible: boolean
  onStopRecording: () => void
}

function RecordingPrompt({ isVisible, onStopRecording }: RecordingPromptProps) {
  return (
    <AnimatePresence>
      {isVisible && (
        <motion.div
          initial={{ top: 0, filter: "blur(5px)" }}
          animate={{
            top: -40,
            filter: "blur(0px)",
            transition: {
              type: "spring",
              filter: { type: "tween" },
            },
          }}
          exit={{ top: 0, filter: "blur(5px)" }}
          className="absolute left-1/2 flex -translate-x-1/2 cursor-pointer overflow-hidden whitespace-nowrap rounded-full border bg-background py-1 text-center text-sm text-muted-foreground"
          onClick={onStopRecording}
        >
          <span className="mx-2.5 flex items-center">
            <Info className="mr-2 h-3 w-3" />
            Click to finish recording
          </span>
        </motion.div>
      )}
    </AnimatePresence>
  )
}

interface RecordingControlsProps {
  isRecording: boolean
  isTranscribing: boolean
  audioStream: MediaStream | null
  textAreaHeight: number
  onStopRecording: () => void
}

function RecordingControls({
  isRecording,
  isTranscribing,
  audioStream,
  textAreaHeight,
  onStopRecording,
}: RecordingControlsProps) {
  if (isRecording) {
    return (
      <div
        className="absolute inset-[1px] z-50 overflow-hidden "
        style={{ height: textAreaHeight - 2 }}
      >
        <AudioVisualizer
          stream={audioStream}
          isRecording={isRecording}
          onClick={onStopRecording}
        />
      </div>
    )
  }

  if (isTranscribing) {
    return (
      <div
        className="absolute inset-[1px] z-50 overflow-hidden "
        style={{ height: textAreaHeight - 2 }}
      >
        <TranscribingOverlay />
      </div>
    )
  }

  return null
}
