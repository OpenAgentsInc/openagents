This is a full transcription of the LangSmith Fleet launch video, including descriptions of the corresponding screens.

https://www.youtube.com/watch?v=t2EYd57rvQg

---

### **00:00 - Introduction**
**Screen Description:** Harrison Chase, CEO of LangChain, is speaking directly to the camera in an office setting.

**Transcript:** 
"Today, we're excited to launch LangSmith Fleet. An enterprise platform for creating, using, and managing your fleet of agents. These agents have their own memory, have access to a collection of tools and skills, and can be exposed through a myriad of channels."

---

### **00:16 - The Chat Interface**
**Screen Description:** The interface of the LangSmith Fleet web application is shown. The user initiates a chat asking the agent to find software engineering candidates in San Francisco. The agent uses search tools and provides a list of results.

**Transcript:** 
"In LangSmith Fleet, everything starts with a chat. You can ask ad-hoc questions and Fleet gets to work, calling tools and doing other actions to accomplish the task that you set out for it. At any point, you can turn a task into an agent with one click."

---

### **00:30 - Agent Types: Claws vs. Assistants**
**Screen Description:** The agent editing screen is displayed. A pop-up menu explains the choice between "Claws" (agents with fixed, autonomous credentials) and "Assistants" (agents that use the credentials of the logged-in user).

**Transcript:** 
"You can create two types of agents: assistants and claws. Assistants act on behalf of you with your credentials. So if an assistant connects to Slack or Notion and you and your teammates are talking to it, it will only see what you have access to see in Slack and Notion. Claws have their own fixed set of credentials. That means that no matter who is interacting with it, it will always have the same permissions. This allows it to be more autonomous, almost having its own identity."

---

### **00:57 - Sharing and Collaboration**
**Screen Description:** The user clicks the "Share" button, showing options to share the agent with the entire workspace or specific people, with permissions to clone, run, or edit.

**Transcript:** 
"In Fleet, you can share your agents with others. You can add people as collaborators or simply let them run your agent."

---

### **01:04 - Multi-Channel Exposure**
**Screen Description:** The interface highlights integration options for channels like Slack, Gmail, and Microsoft Teams.

**Transcript:** 
"You can expose agents through a variety of channels, like Slack and Gmail. This lets you bring your agent to the places where you already work."

---

### **01:12 - Human-in-the-Loop Guardrails**
**Screen Description:** A toggle switch in the tool settings is highlighted, showing the option to require human approval before a specific tool can be executed by an agent.

**Transcript:** 
"Fleet comes with robust human-in-the-loop guardrails. You can add a human-in-the-loop step before any tool call, requiring you to go in and approve that before the agent executes. This helps you be confident that your agents aren't doing potentially dangerous actions without your approval."

---

### **01:27 - Agents Asking for Help**
**Screen Description:** A chat window shows an agent asking the user a clarifying question about their recruitment requirements.

**Transcript:** 
"Agents in Fleet can also ask you, their manager, for help. If agents are stuck, they can ask you any relevant question, get your answer, and then remember that for future iterations."

---

### **01:37 - The Fleet Inbox**
**Screen Description:** The "Inbox" view is shown, displaying various agent tasks that require human attention or have been completed.

**Transcript:** 
"Fleet inbox is a way to manage both these human-in-the-loop interactions as well as agent questions. You can go in, approve any actions, answer any questions, so that they can go on their merry way."

---

### **01:48 - Model and Tool Agnosticism**
**Screen Description:** The chat interface demonstrates how users can switch between different AI models (like Claude or GPT) and manage various tool integrations.

**Transcript:** 
"You can try Fleet for free today at the link below. I'll now go through a deeper walkthrough of the platform and what you can accomplish. Fleet is model-agnostic and tool-agnostic. So you can choose from a variety of models that you have configured in your workspace."

---

### **02:02 - Integrations and MCP Servers**
**Screen Description:** The integrations page shows a library of connected apps and the ability to add custom MCP (Model Context Protocol) servers.

**Transcript:** 
"It's also tool-agnostic. So if you go to the integrations page, you can see a number of built-in tools. You can also add more or add your own custom MCP server."

---

### **02:11 - Tool Authentication**
**Screen Description:** The chat interface shows an OAuth prompt for a Gmail integration, ensuring secure access to user data.

**Transcript:** 
"The chat is the easiest way to get started. You can just ask it to do things. You'll notice that before calling tools, you need to authenticate. We use OAuth to do this in a safe and secure way for each user."

---

### **02:27 - Creating Custom Agents from Chat**
**Screen Description:** The user types "make me an email assistant agent" into the chat. The system walks through a setup process, asking for the email provider and specific goals.

**Transcript:** 
"If you want to do something more than once, you might want to create an agent and save it so it can use the same set of instructions, tools, and skills to do repeated tasks over time. In order to do that, you can just ask the chat to do that. You'll notice that it asks clarifying questions along the way. This helps guide the agent so that it can create the best possible experience for you."

---

### **02:54 - Anatomy of an Agent**
**Screen Description:** The detailed configuration page for the new Email Assistant is shown, including its schedule, channels, core instructions, and toolset.

**Transcript:** 
"So what exactly is an agent? There's a few things. So, there's a schedule that you can give an agent. This is basically a cron job that runs on some schedule with some specific instructions. There's then channels. This is how the agent interacts with the outside world. There's then the core instructions of the agent. And so if you look here, the general purpose agent as it was creating this special agent wrote up a pretty detailed set of instructions. We'll actually see how it can modify those over time. It's then got tools. So these are all done via MCP. It's got sub-agents and skills. And so these are ways of doing specialized tasks."

---

### **03:30 - Defining Identity and Permissions**
**Screen Description:** A modal window appears for confirming the agent's identity type (Claw vs. Assistant). Detailed examples are given for when to use each type.

**Transcript:** 
"Here I want this to trigger on every incoming email, so I'm going to set up an identity. Now there's two types of identities in LangSmith Fleet. First, there's the type where the agent has a fixed set of credentials. So the agent always uses these credentials regardless of who is interacting with it. So for my email assistant, I want it to always answer my emails, regardless of whether Jim emails me or Tom emails me. So I'm going to select fixed set of credentials. The other type of credentials, which I'll show a little bit later on, are user credentials. And we call these assistants. And so this is when the agents act on behalf of the user who is interacting with it. So the cleanest way to think about this is in Slack. If I expose a HR agent in Slack and I message it and Jim messages it, it should get different responses based on who is interacting with it and what it knows about each person. And so user credentials are really good when you want to scope down what the agent does and have it act on behalf of the user every time. Fixed credentials or claws are good when you want this agent to basically have its own identity. In this case, I want it to be acting as my assistant answering my emails."

---

### **04:41 - Advanced Sharing Controls**
**Screen Description:** The screen shows the workspace sharing toggle and individual user permissions being configured.

**Transcript:** 
"Once I create agents, I can share them. So, I can share them with my whole workspace, so everyone in the workspace. And I can give them permissions to either clone the agent, run the agent, which means chat with it, interact with it, or edit the agent. I can also share it with specific people if I want."

---

### **04:57 - Agent Templates**
**Screen Description:** A gallery of pre-built templates is shown, such as LinkedIn Recruiter, Daily Calendar Brief, and Social Media AI Monitor.

**Transcript:** 
"There's a set of templates for agents that you can choose from. So these are some common use cases from social media monitors to LinkedIn recruiters to email assistants to daily briefers that we think you might want to use."

---

### **05:08 - Agent Memory and Self-Editing**
**Screen Description:** An agent is shown editing its own configuration file (`agents.md`) to save a user's location preference. The user has the option to approve or deny this memory update.

**Transcript:** 
"One of the really cool things about LangSmith Fleet is each agent comes with its own memory. This means that when you interact with it, it actually learns and remembers things over time. So let's interact with this and tell it some information that it should remember and use in future searches. I'm going to tell it that I always want candidates in San Francisco. You can see here that it's trying to edit a file. What is this file? This is part of its memory. So this agents.md is the set of instructions that every agent has. And they're all unique for each agent. So when it edits this, it's editing its own memory and it's remembering this: user preferences, location preference, always prioritize candidates in San Francisco. You'll notice that by default it's human-in-the-loop. So letting agents manage their own memory is really powerful but can also be a little dangerous, so by default we have this human-in-the-loop preference. If I want to change that, what I can do is I can go over here, I can click edit, I can go up to this settings tab, and then down here under memory I can toggle this on and off. So now it will always remember things by default without asking me."

---

### **06:18 - Managing Parallel Agent Fleets**
**Screen Description:** The Inbox is used to manage multiple background agents simultaneously, allowing the user to unblock agents by providing approvals.

**Transcript:** 
"You'll notice there's this little inbox thing over here with two next to it. So the whole idea of Fleet is that you have a lot of agents running in parallel in the background, often acting on events. Now we don't think these agents should be fully autonomous. We think that they should ask the user for clarification, we think there should be human-in-the-loop at certain steps. And so how do you manage them? Inbox is the answer. So you can see here a list of all the runs that the agents have done, but you can also filter in to where it needs attention, where it needs approval. So if I go back to this previous chat, I can see this is the chat I had previously. By clicking accept I can now unblock the agent and it goes on its way. So the inbox is a really powerful tool for managing and working with a multitude of agents."

---

### **07:03 - Conclusion**
**Screen Description:** The LangSmith Fleet and LangChain logos appear on a black background.

**Transcript:** 
"So that's a more detailed run-through of LangSmith Fleet. You can try it out for free at the link below."

