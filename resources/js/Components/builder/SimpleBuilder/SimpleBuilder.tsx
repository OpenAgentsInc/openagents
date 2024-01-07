export const SimpleBuilder = () => {
  return (
    <div className="flex grow h-full w-full flex-col items-center">
      <div className="relative flex w-full grow overflow-hidden">
        <div className="flex w-full justify-center md:w-1/2">
          <div className="h-full grow overflow-hidden">
            <div className="flex h-full flex-col px-2 pt-4">
              <div className="grow overflow-hidden">
                <div className="flex h-full grow flex-col overflow-y-auto px-2 pt-6 text-sm">
                  <div className="grow">
                    <div className="mb-6">
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
