import { StyleSheet } from "react-native"
import { typography, fontWeights } from "@/theme/typography"

export const styles = StyleSheet.create({
  container: {
    flex: 1,
  },
  statusBarOverlay: {
    position: 'absolute',
    top: 0,
    left: 0,
    right: 0,
    height: 54,
    backgroundColor: 'rgba(0, 0, 0, 0.5)',
    zIndex: 1,
    overflow: 'hidden',
  },
  scrollView: {
    flex: 1,
  },
  scrollContent: {
    paddingTop: 60,
    paddingBottom: 20,
    paddingHorizontal: 6,
  },
  messageRow: {
    flexDirection: "row",
    marginBottom: 16,
  },
  userMessageRow: {
    justifyContent: "flex-end",
  },
  agentMessageRow: {
    justifyContent: "flex-start",
  },
  messageContainer: {
    paddingVertical: 8,
    paddingHorizontal: 12,
  },
  userMessage: {
    backgroundColor: "#000000",
    borderWidth: 1,
    borderColor: "#FFFFFF",
    maxWidth: "80%",
  },
  agentMessage: {
    borderRadius: 12,
    flex: 1,
    maxWidth: "100%",
  },
  messageText: {
    fontFamily: typography.primary.normal,
    fontWeight: fontWeights.normal,
    fontSize: 13,
    lineHeight: 18,
  },
  codeText: {
    fontFamily: typography.primary.normal,
    fontSize: 12,
    lineHeight: 18,
  },
  userMessageText: {
    color: "#FFFFFF",
  },
  inputContainer: {
    flexDirection: 'row',
    alignItems: 'center',
    borderTopWidth: 1,
    borderTopColor: '#FFFFFF',
    paddingHorizontal: 12,
    paddingVertical: 8,
  },
  input: {
    flex: 1,
    fontFamily: typography.primary.normal,
    fontSize: 13,
    paddingVertical: 8,
    paddingRight: 40,
  },
  sendButton: {
    position: 'absolute',
    right: 12,
    bottom: 12,
    padding: 4,
  },
  codeBlock: {
    backgroundColor: '#000',
    width: '100%',
    borderWidth: 1,
    borderColor: 'white',
    marginTop: -20
  },
})
