import { Link } from '@inertiajs/react';
import * as RadioGroup from '@radix-ui/react-radio-group';
import { useState } from 'react';

export const SimpleBuilder = () => {
  const [selectedValue, setSelectedValue] = useState('create');
  return (
    <div className="flex grow h-full w-full flex-col items-center">
      <div className="relative flex w-full grow overflow-hidden">
        <div className="flex w-full justify-center md:w-1/2">
          <div className="h-full grow overflow-hidden">
            <div className="flex h-full flex-col px-2 pt-4">

              <RadioGroup.Root role="radiogroup" aria-required="false" dir="ltr" className="flex w-full overflow-hidden rounded-xl bg-token-surface-secondary p-1.5 dark:bg-token-surface-tertiary md:w-1/2 mb-2 flex-shrink-0 self-center" tabIndex={0} style={{ outline: 'none' }}
                onValueChange={(value) => {
                  setSelectedValue(value);
                }}
              >
                <RadioGroup.Item type="button" role="radio"
                  aria-checked={selectedValue === 'create'}
                  data-state={selectedValue === 'create' ? "checked" : "unchecked"}
                  value="create"
                  className="text-md w-1/3 flex-grow rounded-lg border-token-border-light p-1.5 font-medium text-token-text-tertiary transition hover:text-token-text-primary radix-state-checked:border radix-state-checked:bg-token-surface-primary radix-state-checked:text-token-text-primary radix-state-checked:shadow-[0_0_2px_rgba(0,0,0,.03)] radix-state-checked:dark:bg-token-surface-secondary md:w-1/2" tabIndex={0} data-radix-collection-item="">
                  Create
                </RadioGroup.Item>
                <RadioGroup.Item type="button" role="radio"
                  aria-checked={selectedValue === 'configure'}
                  data-state={selectedValue === 'configure' ? "checked" : "unchecked"}
                  value="configure" className="text-md w-1/3 flex-grow rounded-lg border-token-border-light p-1.5 font-medium text-token-text-tertiary transition hover:text-token-text-primary radix-state-checked:border radix-state-checked:bg-token-surface-primary radix-state-checked:text-token-text-primary radix-state-checked:shadow-[0_0_2px_rgba(0,0,0,.03)] radix-state-checked:dark:bg-token-surface-secondary md:w-1/2" tabIndex={-1} data-radix-collection-item="">Configure
                </RadioGroup.Item>
                <div className="flex w-1/3 md:hidden"><RadioGroup.Item type="button" role="radio"
                  aria-checked={selectedValue === 'preview'}
                  data-state={selectedValue === 'preview' ? "checked" : "unchecked"}
                  value="preview"
                  className="text-md w-1/3 flex-grow rounded-lg border-token-border-light p-1.5 font-medium text-token-text-tertiary transition hover:text-token-text-primary radix-state-checked:border radix-state-checked:bg-token-surface-primary radix-state-checked:text-token-text-primary radix-state-checked:shadow-[0_0_2px_rgba(0,0,0,.03)] radix-state-checked:dark:bg-token-surface-secondary md:w-1/2" tabIndex={-1} data-radix-collection-item="">
                  Preview
                </RadioGroup.Item>
                </div>
              </RadioGroup.Root>
              <div className="grow overflow-hidden">
                {selectedValue === 'create' && (
                  <div className="h-full w-full pb-5">
                    <div className="relative flex h-full grow overflow-auto">
                      <div className="grow">
                        <div role="presentation" tabIndex={0} className="flex h-full flex-col">
                          <div className="flex-1 overflow-hidden">
                            <div className="react-scroll-to-bottom--css-zyaub-79elbk h-full">
                              <div className="react-scroll-to-bottom--css-zyaub-1n7m0yu">
                                <div className="h-8"></div>
                                <div className="flex flex-col pb-9 text-sm">
                                  <div
                                    className="w-full text-token-text-primary"
                                    data-testid="conversation-turn-1"
                                  >
                                    <div className="px-4 py-2 justify-center text-base md:gap-6 m-auto">
                                      <div className="flex flex-1 text-base mx-auto gap-3 md:px-5 lg:px-1 xl:px-5 md:max-w-3xl lg:max-w-[40rem] xl:max-w-[48rem] group final-completion">
                                        <div className="flex-shrink-0 flex flex-col relative items-end">
                                          <div>
                                            <div className="pt-0.5">
                                              <div className="gizmo-shadow-stroke flex h-6 w-6 items-center justify-center overflow-hidden rounded-full">
                                                <div className="relative p-1 rounded-sm h-9 w-9 text-white flex items-center justify-center"
                                                  style={{
                                                    backgroundColor: 'var(--brand-purple)',
                                                    width: '24px',
                                                    height: '24px'
                                                  }}
                                                >
                                                </div>
                                              </div>
                                            </div>
                                          </div>
                                        </div>
                                        <div className="relative flex w-full flex-col lg:w-[calc(100%-115px)] agent-turn">
                                          <div className="font-semibold select-none">Agent Builder</div>
                                          <div className="flex-col gap-1 md:gap-3">
                                            <div className="flex flex-grow flex-col max-w-full">
                                              <div
                                                data-message-author-role="assistant"
                                                data-message-id="aaa2e8ec-0b49-4b7f-803b-9b1aaf794d6a"
                                                className="min-h-[20px] text-message flex flex-col items-start gap-3 whitespace-pre-wrap break-words [.text-message+&amp;]:mt-5 overflow-x-auto"
                                              >
                                                <div className="markdown prose w-full break-words dark:prose-invert dark">
                                                  <p>
                                                    Hi! I'll help you build a new Agent. You can say something like, "make
                                                    a creative who helps generate visuals for new products" or "make a
                                                    software engineer who helps format my code."
                                                  </p>
                                                  <p>What would you like to make?</p>
                                                </div>
                                              </div>
                                            </div>
                                            <div className="mt-1 flex justify-start gap-3 empty:hidden"></div>
                                          </div>
                                        </div>

                                        {/* <div className="group fixed bottom-3 right-3 z-10 hidden gap-1 lg:flex">
                                          <div className="group relative" data-headlessui-state="">
                                            <button
                                              className="flex items-center justify-center rounded-full border border-gray-200 bg-gray-50 text-gray-600 dark:border-white/10 dark:bg-white/10 dark:text-gray-200"
                                              id="headlessui-menu-button-:r2m:"
                                              type="button"
                                              aria-haspopup="true"
                                              aria-expanded="false"
                                              data-headlessui-state=""
                                            >
                                              <div className="flex h-6 w-6 items-center justify-center text-xs">?</div>
                                            </button>
                                          </div>
                                        </div> */}

                                      </div>

                                    </div>
                                  </div>
                                </div>
                              </div>
                            </div>
                          </div>
                          <div className="w-full pt-2 md:pt-0 dark:border-white/20 md:border-transparent md:dark:border-transparent md:w-[calc(100%-.5rem)]">
                            <form className="stretch mx-2 flex flex-row gap-3 last:mb-2 md:mx-4 md:last:mb-6 lg:mx-auto lg:max-w-2xl xl:max-w-3xl">
                              <div className="relative flex h-full flex-1 items-stretch md:flex-col">
                                <div className="flex w-full items-center">
                                  <div className="overflow-hidden [&amp;:has(textarea:focus)]:border-token-border-xheavy [&amp;:has(textarea:focus)]:shadow-[0_2px_6px_rgba(0,0,0,.05)] flex flex-col w-full dark:border-token-border-heavy flex-grow relative border border-token-border-heavy dark:text-white rounded-2xl bg-white dark:bg-gray-800 shadow-[0_0_0_2px_rgba(255,255,255,0.95)] dark:shadow-[0_0_0_2px_rgba(52,53,65,0.95)]">
                                    <textarea
                                      id="prompt-textarea"
                                      tabIndex={0}
                                      data-id="6831a799-1f41-4d29-bf6d-a41af1c37faf"
                                      rows={1}
                                      placeholder="Message Agent Builder…"
                                      className="m-0 w-full resize-none border-0 bg-transparent py-[10px] pr-10 focus:ring-0 focus-visible:ring-0 dark:bg-transparent md:py-3.5 md:pr-12 placeholder-black/50 dark:placeholder-white/50 pl-10 md:pl-[55px]"
                                      style={{ maxHeight: '200px', height: '52px', overflowY: 'hidden' }}
                                    ></textarea>
                                    <div className="absolute bottom-2 md:bottom-3 left-2 md:left-4">
                                      <div className="flex">
                                        <button className="btn relative p-0 text-black dark:text-white" aria-label="Attach files">
                                          <div className="flex w-full gap-2 items-center justify-center">
                                            <svg
                                              width="24"
                                              height="24"
                                              viewBox="0 0 24 24"
                                              fill="none"
                                              xmlns="http://www.w3.org/2000/svg"
                                            >
                                              <path
                                                fillRule="evenodd"
                                                clipRule="evenodd"
                                                d="M9 7C9 4.23858 11.2386 2 14 2C16.7614 2 19 4.23858 19 7V15C19 18.866 15.866 22 12 22C8.13401 22 5 18.866 5 15V9C5 8.44772 5.44772 8 6 8C6.55228 8 7 8.44772 7 9V15C7 17.7614 9.23858 20 12 20C14.7614 20 17 17.7614 17 15V7C17 5.34315 15.6569 4 14 4C12.3431 4 11 5.34315 11 7V15C11 15.5523 11.4477 16 12 16C12.5523 16 13 15.5523 13 15V9C13 8.44772 13.4477 8 14 8C14.5523 8 15 8.44772 15 9V15C15 16.6569 13.6569 18 12 18C10.3431 18 9 16.6569 9 15V7Z"
                                                fill="currentColor"
                                              ></path>
                                            </svg>
                                          </div>
                                        </button>
                                        <input multiple={false} type="file" tabIndex={-1} className="hidden" style={{ display: "none" }} />
                                      </div>
                                    </div>
                                    <button
                                      disabled={false}
                                      className="absolute md:bottom-3 md:right-3 dark:hover:bg-gray-900 dark:disabled:hover:bg-transparent right-2 dark:disabled:bg-white disabled:bg-black disabled:opacity-10 disabled:text-gray-400 enabled:bg-black text-white p-0.5 border border-black rounded-lg dark:border-white dark:bg-white bottom-1.5 transition-colors"
                                      data-testid="send-button"
                                    >
                                      <span className="" data-state="closed">
                                        <svg width="24" height="24" viewBox="0 0 24 24" fill="none" className="text-white dark:text-black">
                                          <path
                                            d="M7 11L12 6L17 11M12 18V7"
                                            stroke="currentColor"
                                            strokeWidth="2"
                                            strokeLinecap="round"
                                            strokeLinejoin="round"
                                          ></path>
                                        </svg>
                                      </span>
                                    </button>
                                  </div>
                                </div>
                              </div>
                            </form>
                            <div className="relative px-2 py-2 text-center text-xs text-gray-600 dark:text-gray-300 md:px-[60px]"></div>
                          </div>
                        </div>
                      </div>
                    </div>
                  </div>
                )}
                {selectedValue === 'configure' && (
                  <div className="flex h-full grow flex-col overflow-y-auto px-2 pt-6 text-sm">
                    <div className="grow">
                      <div className="mb-6">
                        {/* <div className="flex w-full items-center justify-center gap-4">
                          <button
                            type="button"
                            id="radix-:r2a:"
                            aria-haspopup="menu"
                            aria-expanded="false"
                            data-state="closed"
                            className="h-20 w-20"
                          >
                            <div className="flex h-full w-full items-center justify-center rounded-full border-2 border-dashed border-black border-token-border-medium">
                              <svg
                                stroke="currentColor"
                                fill="none"
                                strokeWidth="2"
                                viewBox="0 0 24 24"
                                strokeLinecap="round"
                                strokeLinejoin="round"
                                className="text-4xl"
                                height="1em"
                                width="1em"
                                xmlns="http://www.w3.org/2000/svg"
                              >
                                <line x1="12" y1="5" x2="12" y2="19"></line>
                                <line x1="5" y1="12" x2="19" y2="12"></line>
                              </svg>
                            </div>
                          </button>
                        </div> */}
                        <div className="mb-1.5 flex items-center">
                          <span className="" data-state="closed">
                            <label className="block font-medium text-token-text-primary">Name</label>
                          </span>
                        </div>
                        <input
                          type="text"
                          placeholder="Name your Agent"
                          className="w-full resize-none overflow-y-auto rounded-lg px-3 py-2 text-sm outline-none focus:ring-2 border focus:ring-blue-400 border-token-border-medium h-9 dark:bg-gray-800"
                          value=""
                        />
                      </div>
                      <div className="mb-6 mt-4">
                        <div className="mb-1.5 flex items-center">
                          <span className="" data-state="closed">
                            <label className="block font-medium text-token-text-primary">Description</label>
                          </span>
                        </div>
                        <input
                          type="text"
                          placeholder="Add a short description about what this Agent does"
                          className="w-full resize-none overflow-y-auto rounded-lg px-3 py-2 text-sm outline-none focus:ring-2 border focus:ring-blue-400 border-token-border-medium h-9 dark:bg-gray-800"
                          value=""
                        />
                      </div>
                      <div className="mb-6">
                        <div className="mb-1.5 flex items-center">
                          <span className="" data-state="closed">
                            <label className="block font-medium text-token-text-primary">Instructions</label>
                          </span>
                        </div>
                        <div className="relative">
                          <textarea
                            className="w-full text-sm overflow-y-auto rounded-lg border px-3 py-2 focus:ring-2 focus:ring-blue-400 border-token-border-medium dark:bg-gray-800 bg-white h-32 resize-none"
                            rows={8}
                            placeholder="What does this Agent do? How does it behave? What should it avoid doing?"
                          ></textarea>
                          <button className="absolute bottom-3 right-2 text-token-text-tertiary">
                            <svg
                              width="24"
                              height="24"
                              viewBox="0 0 24 24"
                              fill="none"
                              xmlns="http://www.w3.org/2000/svg"
                              className="icon-sm"
                            >
                              <path
                                fillRule="evenodd"
                                clipRule="evenodd"
                                d="M13 5C13 4.44772 13.4477 4 14 4H19C19.5523 4 20 4.44772 20 5V10C20 10.5523 19.5523 11 19 11C18.4477 11 18 10.5523 18 10V7.41421L14.7071 10.7071C14.3166 11.0976 13.6834 11.0976 13.2929 10.7071C12.9024 10.3166 12.9024 9.68342 13.2929 9.29289L16.5858 6H14C13.4477 6 13 5.55228 13 5ZM5 13C5.55228 13 6 13.4477 6 14V16.5858L9.29289 13.2929C9.68342 12.9024 10.3166 12.9024 10.7071 13.2929C11.0976 13.6834 11.0976 14.3166 10.7071 14.7071L7.41421 18H10C10.5523 18 11 18.4477 11 19C11 19.5523 10.5523 20 10 20H5C4.44772 20 4 19.5523 4 19V14C4 13.4477 4.44772 13 5 13Z"
                                fill="currentColor"
                              ></path>
                            </svg>
                          </button>
                        </div>
                      </div>
                      <div className="mb-6 hidden">
                        <div className="mb-1.5 flex items-center">
                          <span className="" data-state="closed">
                            <label className="block font-medium text-token-text-primary">Welcome Message</label>
                          </span>
                        </div>
                        <div className="relative">
                          <textarea
                            className="w-full text-sm overflow-y-auto rounded-lg border px-3 py-2 focus:ring-2 focus:ring-blue-400 border-token-border-medium dark:bg-gray-800 bg-white h-16 resize-none"
                            placeholder="How this Agent starts conversations."
                          ></textarea>

                          <button className="absolute bottom-3 right-2 text-token-text-tertiary"><svg width="24" height="24" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg" className="icon-sm"><path fillRule="evenodd" clipRule="evenodd" d="M13 5C13 4.44772 13.4477 4 14 4H19C19.5523 4 20 4.44772 20 5V10C20 10.5523 19.5523 11 19 11C18.4477 11 18 10.5523 18 10V7.41421L14.7071 10.7071C14.3166 11.0976 13.6834 11.0976 13.2929 10.7071C12.9024 10.3166 12.9024 9.68342 13.2929 9.29289L16.5858 6H14C13.4477 6 13 5.55228 13 5ZM5 13C5.55228 13 6 13.4477 6 14V16.5858L9.29289 13.2929C9.68342 12.9024 10.3166 12.9024 10.7071 13.2929C11.0976 13.6834 11.0976 14.3166 10.7071 14.7071L7.41421 18H10C10.5523 18 11 18.4477 11 19C11 19.5523 10.5523 20 10 20H5C4.44772 20 4 19.5523 4 19V14C4 13.4477 4.44772 13 5 13Z" fill="currentColor"></path></svg></button>
                        </div>
                      </div>
                      <div className="mb-6">

                        <div className="mb-1.5 flex items-center">
                          <span className="" data-state="closed">
                            <label className="block font-medium text-token-text-primary">Conversation starters</label>
                          </span>
                        </div>
                        <div className="flex flex-col gap-1">
                          <div className="flex items-center">
                            <input
                              className="w-full resize-none overflow-y-auto rounded-lg px-3 py-2 text-sm outline-none focus:ring-2 border focus:ring-blue-400 border-token-border-medium h-9 dark:bg-gray-800 rounded-r-none"
                              type="text"
                              value=""
                            />
                            <button className="flex h-9 w-9 items-center justify-center rounded-lg rounded-l-none border border-l-0 border-token-border-medium">
                              <svg
                                width="24"
                                height="24"
                                viewBox="0 0 24 24"
                                fill="none"
                                xmlns="http://www.w3.org/2000/svg"
                                className="icon-sm"
                              >
                                <path
                                  d="M6.34315 6.34338L17.6569 17.6571M17.6569 6.34338L6.34315 17.6571"
                                  stroke="currentColor"
                                  strokeWidth="2"
                                  strokeLinecap="round"
                                  strokeLinejoin="round"
                                ></path>
                              </svg>
                            </button>
                          </div>
                        </div>
                      </div>
                      <div className="mb-6">
                        <div className="mb-1.5 flex items-center">
                          <span className="" data-state="closed">
                            <label className="block font-medium text-token-text-primary">Knowledge</label>
                          </span>
                        </div>
                        <div className="flex flex-col gap-4">
                          <div className="rounded-lg text-gray-500">
                            If you upload files under Knowledge, conversations with your Agent may include file contents.
                          </div>
                          <div>
                            <button className="btn relative btn-neutral h-8 rounded-lg border-token-border-light font-medium">
                              <div className="flex w-full gap-2 items-center justify-center">
                                <input multiple={false} type="file" tabIndex={-1} style={{ display: 'none' }} />
                                Upload files
                              </div>
                            </button>
                          </div>
                        </div>
                      </div>
                      {/* <div className="mb-6">
                        <div className="mb-1.5 flex items-center">
                          <span className="" data-state="closed">
                            <label className="block font-medium text-token-text-primary">Capabilities</label>
                          </span>
                        </div>
                        <div className="flex flex-col items-start gap-2">
                          <div className="form-check flex items-center">
                            <input
                              className="form-check-input float-left mr-2 h-4 w-4 cursor-pointer appearance-none rounded-sm border border-gray-300 bg-white bg-contain bg-center bg-no-repeat align-top transition duration-200 checked:border-blue-600 checked:bg-blue-600 focus:outline-none !rounded border-gray-950 checked:!bg-black dark:border-gray-600 dark:bg-gray-700"
                              type="checkbox"
                              id="browser"
                              checked={false}
                            />
                            <label className="form-check-label text-gray-800 dark:text-gray-100 w-full cursor-pointer" htmlFor="browser">
                              Web Browsing
                            </label>
                          </div>
                          <div className="form-check flex items-center">
                            <input
                              className="form-check-input float-left mr-2 h-4 w-4 cursor-pointer appearance-none rounded-sm border border-gray-300 bg-white bg-contain bg-center bg-no-repeat align-top transition duration-200 checked:border-blue-600 checked:bg-blue-600 focus:outline-none !rounded border-gray-950 checked:!bg-black dark:border-gray-600 dark:bg-gray-700"
                              type="checkbox"
                              id="dalle"
                              checked={false}
                            />
                            <label className="form-check-label text-gray-800 dark:text-gray-100 w-full cursor-pointer" htmlFor="dalle">
                              Image Generation
                            </label>
                          </div>
                        </div>
                      </div>
                      <div className="mb-6">
                        <div className="mb-1.5 flex items-center">
                          <span className="" data-state="closed">
                            <label className="block font-medium text-token-text-primary">Actions</label>
                          </span>
                        </div>
                        <div className="space-y-1">
                          <button className="btn relative btn-neutral h-8 rounded-lg border-token-border-light font-medium mt-2">
                            <div className="flex w-full gap-2 items-center justify-center">Create new action</div>
                          </button>
                        </div>
                      </div> */}
                    </div>
                  </div>
                )}
              </div>
            </div>
          </div>
        </div>
        <div className="hidden w-1/2 justify-center border-l border-token-border-medium bg-token-surface-secondary pt-4 md:flex">
          <div className="flex-grow pb-5">
            <div className="h-full">
              <div className="flex h-full w-full">
                <div className="flex grow flex-col">
                  <div className="relative mb-2 flex-shrink-0">
                    <div className="flex justify-center py-1">
                      <div className="group flex items-center gap-2 text-lg font-medium">
                        <div className="icon-md"></div>
                        <button className="flex items-center gap-2">
                          Preview
                          <div className="text-token-text-primary" style={{ transform: "none" }}>
                            <svg width="24" height="24" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg" className="icon-md invisible group-hover:visible">
                              <path d="M4.5 3.5V8H9" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"></path>
                              <path d="M4.5 7.99645C5.93143 5.3205 8.75312 3.5 12 3.5C16.6944 3.5 20.5 7.30558 20.5 12C20.5 16.6944 16.6944 20.5 12 20.5C7.6439 20.5 4.05313 17.2232 3.5582 13" stroke="currentColor" strokeWidth="2" strokeLinecap="round"></path>
                            </svg>
                          </div>
                        </button>
                      </div>
                    </div>
                  </div>
                  <div className="relative grow overflow-auto px-2">
                    <div role="presentation" className="flex h-full flex-col"><div className="flex-1 overflow-hidden"><div className="relative h-full w-full"><div className="absolute left-0 top-0 h-full w-full"><div className="flex h-full flex-col items-center justify-center"><div className="relative"><div className="mb-3 h-[72px] w-[72px]"><div className="gizmo-shadow-stroke relative flex h-full items-center justify-center rounded-full bg-white text-black"><svg stroke="currentColor" fill="none" strokeWidth="2" viewBox="0 0 24 24" strokeLinecap="round" strokeLinejoin="round" className="text-token-secondary h-2/3 w-2/3" height="1em" width="1em" xmlns="http://www.w3.org/2000/svg"><path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"></path><polyline points="3.27 6.96 12 12.01 20.73 6.96"></polyline><line x1="12" y1="22.08" x2="12" y2="12"></line></svg></div></div></div><div className="flex flex-col items-center gap-0 p-2"><div className="text-center text-2xl font-medium"></div></div></div></div></div></div><div className="w-full pt-2 md:pt-0 dark:border-white/20 md:border-transparent md:dark:border-transparent md:w-[calc(100%-.5rem)]"><form className="stretch mx-2 flex flex-row gap-3 last:mb-2 md:mx-4 md:last:mb-6 lg:mx-auto lg:max-w-2xl xl:max-w-3xl"><div className="relative flex h-full flex-1 items-stretch md:flex-col"><div className="flex w-full items-center"><div className="overflow-hidden [&amp;:has(textarea:focus)]:border-token-border-xheavy [&amp;:has(textarea:focus)]:shadow-[0_2px_6px_rgba(0,0,0,.05)] flex flex-col w-full dark:border-token-border-heavy flex-grow relative border border-token-border-heavy dark:text-white rounded-2xl bg-white dark:bg-gray-800 shadow-[0_0_0_2px_rgba(255,255,255,0.95)] dark:shadow-[0_0_0_2px_rgba(52,53,65,0.95)]"><textarea id="prompt-textarea" tabIndex={0} data-id="root" rows={1} placeholder="Message Agent…" className="m-0 w-full resize-none border-0 bg-transparent py-[10px] pr-10 focus:ring-0 focus-visible:ring-0 dark:bg-transparent md:py-3.5 md:pr-12 placeholder-black/50 dark:placeholder-white/50 pl-3 md:pl-4"
                      style={{ maxHeight: '200px', height: '52px', overflowY: 'hidden' }}

                    ></textarea><button disabled={false} className="absolute md:bottom-3 md:right-3 dark:hover:bg-gray-900 dark:disabled:hover:bg-transparent right-2 dark:disabled:bg-white disabled:bg-black disabled:opacity-10 disabled:text-gray-400 enabled:bg-black text-white p-0.5 border border-black rounded-lg dark:border-white dark:bg-white bottom-1.5 transition-colors" data-testid="send-button"><span className="" data-state="closed"><svg width="24" height="24" viewBox="0 0 24 24" fill="none" className="text-white dark:text-black"><path d="M7 11L12 6L17 11M12 18V7" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"></path></svg></span></button></div></div></div></form><div className="relative px-2 py-2 text-center text-xs text-gray-600 dark:text-gray-300 md:px-[60px]"></div></div></div>
                    {/* <div className="group fixed bottom-3 right-3 z-10 hidden gap-1 lg:flex"><div className="group relative" data-headlessui-state=""><button className="flex items-center justify-center rounded-full border border-gray-200 bg-gray-50 text-gray-600 dark:border-white/10 dark:bg-white/10 dark:text-gray-200" id="headlessui-menu-button-:rg:" type="button" aria-haspopup="true" aria-expanded="false" data-headlessui-state=""><div className="flex h-6 w-6 items-center justify-center text-xs">?</div></button></div></div> */}
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div >
  )
}
