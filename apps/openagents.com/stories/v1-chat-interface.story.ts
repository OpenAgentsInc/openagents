export const title = "OpenAgents v1 - Exact Chat Interface"
export const component = "OpenAgents v1 Chat Recreation"

export const FullV1Interface = {
  name: "Full v1 Chat Interface",
  html: `
    <link rel="stylesheet" href="/tailwind/components/chat/v1-interface.css">

    <div class="v1-chat-container">
      <!-- Header with user menu -->
      <div class="v1-chat-header">
        <div style="color: var(--v1-gray); cursor: pointer;">
          <!-- User menu would go here -->
        </div>
      </div>

      <div class="v1-chat-layout">
        <!-- Sidebar -->
        <div class="v1-sidebar open">
          <div class="v1-sidebar-inner">
            <div class="v1-sidebar-header">
              <button class="v1-menu-button">
                <svg class="h-6 w-6" viewBox="0 0 20 20" fill="none">
                  <path d="M2.5 5.83333H17.5M2.5 10H17.5M2.5 14.1667H17.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
                </svg>
              </button>
              <a href="/chat" class="v1-new-chat-button">
                <svg class="h-6 w-6" viewBox="0 0 20 20" fill="none">
                  <path d="M10 4V16M4 10H16" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
                </svg>
              </a>
            </div>

            <div class="v1-threads-container">
              <div class="v1-threads-list">
                <div>
                  <ol>
                    <!-- Active thread -->
                    <li class="v1-thread-item">
                      <div class="v1-thread-link-wrapper active">
                        <a href="/chat/1" class="v1-thread-title">New chat</a>
                        <div class="v1-thread-actions">
                          <button style="background: none; border: none; color: currentColor; cursor: pointer;">
                            <svg class="w-4 h-4" viewBox="0 0 16 16" fill="none">
                              <circle cx="8" cy="3" r="1.5" fill="currentColor"/>
                              <circle cx="8" cy="8" r="1.5" fill="currentColor"/>
                              <circle cx="8" cy="13" r="1.5" fill="currentColor"/>
                            </svg>
                          </button>
                        </div>
                      </div>
                    </li>

                    <!-- Other threads -->
                    <li class="v1-thread-item">
                      <div class="v1-thread-link-wrapper">
                        <a href="/chat/2" class="v1-thread-title">Python fibonacci function</a>
                        <div class="v1-thread-actions">
                          <button style="background: none; border: none; color: currentColor; cursor: pointer;">
                            <svg class="w-4 h-4" viewBox="0 0 16 16" fill="none">
                              <circle cx="8" cy="3" r="1.5" fill="currentColor"/>
                              <circle cx="8" cy="8" r="1.5" fill="currentColor"/>
                              <circle cx="8" cy="13" r="1.5" fill="currentColor"/>
                            </svg>
                          </button>
                        </div>
                      </div>
                    </li>
                  </ol>
                </div>
              </div>

              <!-- Sidebar Footer -->
              <div class="v1-sidebar-footer">
                <ol style="list-style: none; padding: 0; margin: 0;">
                  <li>
                    <div style="position: relative; z-index: 15;">
                      <div style="position: relative; border-radius: 0.5rem; padding: 0 0.75rem;">
                        <a class="v1-sidebar-link" href="/store">
                          <div class="v1-sidebar-link-text">Agent Store</div>
                          <div class="v1-beta-badge">Beta</div>
                        </a>
                      </div>
                    </div>
                  </li>
                  <li>
                    <div style="position: relative; z-index: 15;">
                      <div style="position: relative; border-radius: 0.5rem; padding: 0 0.75rem;">
                        <a class="v1-sidebar-link" href="/plugins">
                          <div class="v1-sidebar-link-text">Plugins</div>
                          <div class="v1-beta-badge">Beta</div>
                        </a>
                      </div>
                    </div>
                  </li>
                  <li>
                    <div style="position: relative; z-index: 15;">
                      <div style="position: relative; border-radius: 0.5rem; padding: 0 0.75rem;">
                        <a class="v1-sidebar-link" href="/blog">
                          <div class="v1-sidebar-link-text">Blog</div>
                        </a>
                      </div>
                    </div>
                  </li>
                  <li>
                    <div style="position: relative; z-index: 15;">
                      <div style="position: relative; border-radius: 0.5rem; padding: 0 0.75rem;">
                        <a class="v1-sidebar-link" href="/changelog">
                          <div class="v1-sidebar-link-text">Changelog</div>
                        </a>
                      </div>
                    </div>
                  </li>
                  <li>
                    <div style="position: relative; z-index: 15;">
                      <div style="position: relative; border-radius: 0.5rem; padding: 0 0.75rem;">
                        <a class="v1-sidebar-link" href="https://openagents.com/docs" target="_blank">
                          <div class="v1-sidebar-link-text">Docs & guides</div>
                          <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                            <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"></path>
                            <polyline points="15 3 21 3 21 9"></polyline>
                            <line x1="10" y1="14" x2="21" y2="3"></line>
                          </svg>
                        </a>
                      </div>
                    </div>
                  </li>
                  <li>
                    <div style="position: relative; z-index: 15;">
                      <div style="position: relative; border-radius: 0.5rem; padding: 0 0.75rem;">
                        <a class="v1-sidebar-link" href="https://stacker.news/~openagents" target="_blank">
                          <div class="v1-sidebar-link-text">Community</div>
                          <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                            <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"></path>
                            <polyline points="15 3 21 3 21 9"></polyline>
                            <line x1="10" y1="14" x2="21" y2="3"></line>
                          </svg>
                        </a>
                      </div>
                    </div>
                  </li>
                  <li>
                    <div style="position: relative; z-index: 15;">
                      <div style="position: relative; border-radius: 0.5rem; padding: 0 0.75rem;">
                        <a class="v1-sidebar-link" href="https://github.com/OpenAgentsInc/openagents" target="_blank">
                          <div class="v1-sidebar-link-text">Source code</div>
                          <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                            <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"></path>
                            <polyline points="15 3 21 3 21 9"></polyline>
                            <line x1="10" y1="14" x2="21" y2="3"></line>
                          </svg>
                        </a>
                      </div>
                    </div>
                  </li>
                  <li>
                    <div style="position: relative; z-index: 15;">
                      <div style="position: relative; border-radius: 0.5rem; padding: 0 0.75rem;">
                        <a class="v1-sidebar-link" href="/upgrade">
                          <div class="v1-sidebar-link-text">Upgrade to Pro</div>
                        </a>
                      </div>
                    </div>
                  </li>
                  <li>
                    <div class="v1-sidebar-footer-bottom">
                      <a href="/terms" class="v1-footer-link">Terms</a>
                      <span class="v1-footer-separator">·</span>
                      <a href="/privacy" class="v1-footer-link">Privacy</a>
                    </div>
                  </li>
                </ol>
              </div>
            </div>
          </div>
        </div>

        <!-- Main Chat Area -->
        <div class="v1-chat-main sidebar-open">
          <div class="v1-chat-content">
            <div class="v1-model-selector-bar">
              <div class="v1-model-selector">
                <span>OpenAgents</span>
              </div>
            </div>

            <div class="v1-messages-container">
              <div class="v1-messages-wrapper">
                <div class="v1-messages-list">
                  <!-- User Message -->
                  <div class="v1-message">
                    <div class="v1-message-inner">
                      <div class="v1-message-content">
                        <div class="v1-message-avatar">
                          <div class="v1-avatar-icon user-icon">
                            <svg width="100%" height="100%" viewBox="0 0 48 48" fill="none">
                              <path d="M24 8C19.5807 8 16 11.5807 16 16C16 20.4193 19.5807 24 24 24C28.4193 24 32 20.4193 32 16C32 11.5807 28.4193 8 24 8ZM24 28C11.7995 28 8 36 8 36V40H40V36C40 36 36.2005 28 24 28Z" fill="currentColor"/>
                            </svg>
                          </div>
                        </div>
                        <div class="v1-message-body">
                          <span class="v1-message-author">You</span>
                          <div class="v1-message-text">Can you help me write a Python function to calculate fibonacci numbers?</div>
                        </div>
                      </div>
                    </div>
                  </div>

                  <!-- Bot Message -->
                  <div class="v1-message">
                    <div class="v1-message-inner">
                      <div class="v1-message-content">
                        <div class="v1-message-avatar">
                          <div class="v1-avatar-icon bot-icon">
                            <svg width="100%" height="100%" viewBox="0 0 20 20" fill="none">
                              <path d="M4.82571 2.79657L4.8219 2.79911C2.19461 4.54501 0.625 7.46727 0.625 10.6211C0.625 15.7957 4.83214 20.0001 10 20.0001C15.1679 20.0001 19.375 15.7957 19.375 10.6211C19.375 7.46724 17.8054 4.54493 15.1751 2.79903L15.1744 2.79856C14.6737 2.46848 13.9936 2.60312 13.6608 3.11074C13.325 3.61575 13.4638 4.29839 13.9661 4.63329L13.9666 4.63367C15.9819 5.96999 17.1834 8.20863 17.1834 10.6211C17.1834 14.5783 13.9623 17.8034 10 17.8034C6.03772 17.8034 2.81656 14.5783 2.81656 10.6211C2.81656 8.20863 4.01814 5.96999 6.03337 4.63367L6.03394 4.63329C6.5361 4.29846 6.67495 3.61603 6.33936 3.11105C6.1331 2.79489 5.78727 2.62021 5.4373 2.61942C5.2228 2.61607 5.01139 2.67772 4.82571 2.79657Z" fill="white"/>
                              <path d="M10 9.91821e-05C9.39721 9.91821e-05 8.90422 0.489769 8.90422 1.09843V8.55099C8.90422 9.15965 9.39721 9.64932 10 9.64932C10.6028 9.64932 11.0958 9.15965 11.0958 8.55099V1.09843C11.0958 0.489769 10.6028 9.91821e-05 10 9.91821e-05Z" fill="white"/>
                            </svg>
                          </div>
                        </div>
                        <div class="v1-message-body">
                          <span class="v1-message-author">OpenAgents</span>
                          <div class="v1-message-text">I'll help you create a Python function to calculate Fibonacci numbers. Here's an efficient implementation:

<pre style="background: #1e1e1e; padding: 1rem; border-radius: 0.25rem; margin: 1rem 0; overflow-x: auto;">
<code style="color: #D7D8E5;">def fibonacci(n):
    if n <= 1:
        return n
    a, b = 0, 1
    for _ in range(2, n + 1):
        a, b = b, a + b
    return b</code>
</pre>

This function uses iteration rather than recursion, making it more efficient for larger numbers.</div>
                        </div>
                      </div>
                    </div>
                  </div>

                  <!-- Streaming Message -->
                  <div class="v1-message">
                    <div class="v1-message-inner">
                      <div class="v1-message-content">
                        <div class="v1-message-avatar">
                          <div class="v1-avatar-icon bot-icon">
                            <svg width="100%" height="100%" viewBox="0 0 20 20" fill="none">
                              <path d="M4.82571 2.79657L4.8219 2.79911C2.19461 4.54501 0.625 7.46727 0.625 10.6211C0.625 15.7957 4.83214 20.0001 10 20.0001C15.1679 20.0001 19.375 15.7957 19.375 10.6211C19.375 7.46724 17.8054 4.54493 15.1751 2.79903L15.1744 2.79856C14.6737 2.46848 13.9936 2.60312 13.6608 3.11074C13.325 3.61575 13.4638 4.29839 13.9661 4.63329L13.9666 4.63367C15.9819 5.96999 17.1834 8.20863 17.1834 10.6211C17.1834 14.5783 13.9623 17.8034 10 17.8034C6.03772 17.8034 2.81656 14.5783 2.81656 10.6211C2.81656 8.20863 4.01814 5.96999 6.03337 4.63367L6.03394 4.63329C6.5361 4.29846 6.67495 3.61603 6.33936 3.11105C6.1331 2.79489 5.78727 2.62021 5.4373 2.61942C5.2228 2.61607 5.01139 2.67772 4.82571 2.79657Z" fill="white"/>
                              <path d="M10 9.91821e-05C9.39721 9.91821e-05 8.90422 0.489769 8.90422 1.09843V8.55099C8.90422 9.15965 9.39721 9.64932 10 9.64932C10.6028 9.64932 11.0958 9.15965 11.0958 8.55099V1.09843C11.0958 0.489769 10.6028 9.91821e-05 10 9.91821e-05Z" fill="white"/>
                            </svg>
                          </div>
                        </div>
                        <div class="v1-message-body">
                          <span class="v1-message-author">OpenAgents</span>
                          <div class="v1-dot-flashing"></div>
                        </div>
                      </div>
                    </div>
                  </div>
                </div>
              </div>
            </div>

            <!-- Input Area -->
            <div class="v1-input-area">
              <div class="v1-input-container">
                <form class="v1-input-form">
                  <div class="v1-textarea-wrapper">
                    <textarea
                      class="v1-chat-input"
                      placeholder="Message OpenAgents..."
                      rows="1"
                    ></textarea>
                    <button type="submit" class="v1-send-button">
                      <svg width="24" height="24" viewBox="0 0 24 24" fill="none">
                        <path d="M7 11L12 6L17 11M12 18V7" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
                      </svg>
                    </button>
                  </div>
                </form>
                <div class="v1-input-footer">
                  <span>Free tier: 10 messages remaining today</span>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  `,
  description: "Exact recreation of OpenAgents v1 chat interface with all original styling"
}

export const V1SidebarClosed = {
  name: "v1 Interface - Sidebar Closed",
  html: `
    <link rel="stylesheet" href="/tailwind/components/chat/v1-interface.css">

    <div class="v1-chat-container" style="height: 400px;">
      <div class="v1-chat-layout">
        <!-- Closed Sidebar -->
        <div class="v1-sidebar closed">
          <div class="v1-sidebar-inner">
            <div class="v1-sidebar-header">
              <button class="v1-menu-button">
                <svg class="h-6 w-6" viewBox="0 0 20 20" fill="none">
                  <path d="M2.5 5.83333H17.5M2.5 10H17.5M2.5 14.1667H17.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
                </svg>
              </button>
            </div>
          </div>
        </div>

        <!-- Main Chat Area -->
        <div class="v1-chat-main sidebar-closed">
          <div class="v1-chat-content">
            <div class="v1-model-selector-bar">
              <div class="v1-model-selector">
                <span>OpenAgents</span>
              </div>
            </div>

            <div class="v1-messages-container">
              <div class="v1-messages-wrapper">
                <div class="v1-messages-list">
                  <div style="text-align: center; padding: 2rem; color: var(--v1-gray);">
                    Chat interface with sidebar closed
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  `,
  description: "v1 interface with sidebar closed state"
}

export const V1ColorPalette = {
  name: "v1 Color Palette",
  html: `
    <div style="background: #000; padding: 2rem; font-family: 'JetBrains Mono', monospace;">
      <h3 style="color: #fff; margin-bottom: 1rem;">OpenAgents v1 Colors</h3>
      <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 1rem;">
        <div style="background: #000000; border: 1px solid #3D3D40; padding: 1rem; border-radius: 0.25rem;">
          <div style="color: #D7D8E5;">Black: #000000</div>
          <div style="color: #8B8585; font-size: 0.875rem;">Background</div>
        </div>
        <div style="background: #1e1e1e; border: 1px solid #3D3D40; padding: 1rem; border-radius: 0.25rem;">
          <div style="color: #D7D8E5;">Off-black: #1e1e1e</div>
          <div style="color: #8B8585; font-size: 0.875rem;">Subtle backgrounds</div>
        </div>
        <div style="background: #262626; border: 1px solid #3D3D40; padding: 1rem; border-radius: 0.25rem;">
          <div style="color: #D7D8E5;">Active: #262626</div>
          <div style="color: #8B8585; font-size: 0.875rem;">Active thread bg</div>
        </div>
        <div style="background: #000; border: 1px solid #3D3D40; padding: 1rem; border-radius: 0.25rem;">
          <div style="color: #3D3D40;">Dark Gray: #3D3D40</div>
          <div style="color: #8B8585; font-size: 0.875rem;">Borders</div>
        </div>
        <div style="background: #000; border: 1px solid #3D3D40; padding: 1rem; border-radius: 0.25rem;">
          <div style="color: #8B8585;">Gray: #8B8585</div>
          <div style="color: #8B8585; font-size: 0.875rem;">Secondary text</div>
        </div>
        <div style="background: #000; border: 1px solid #3D3D40; padding: 1rem; border-radius: 0.25rem;">
          <div style="color: #A7A7A7;">Light Gray: #A7A7A7</div>
          <div style="color: #8B8585; font-size: 0.875rem;">Tertiary text</div>
        </div>
        <div style="background: #000; border: 1px solid #3D3D40; padding: 1rem; border-radius: 0.25rem;">
          <div style="color: #D7D8E5;">Text: #D7D8E5</div>
          <div style="color: #8B8585; font-size: 0.875rem;">Main text</div>
        </div>
        <div style="background: #000; border: 1px solid #3D3D40; padding: 1rem; border-radius: 0.25rem;">
          <div style="color: #ffffff;">White: #ffffff</div>
          <div style="color: #8B8585; font-size: 0.875rem;">Headings, buttons</div>
        </div>
      </div>
    </div>
  `,
  description: "Complete v1 color system"
}
