import { StyleSheet } from "react-native"
import { colors } from "@/theme/colors"
import { typography } from "@/theme/typography"

export const styles = StyleSheet.create({
  // Modal base styles
  modalContainer: {
    flex: 1,
    backgroundColor: "rgba(0,0,0,0.85)",
    paddingHorizontal: 10
  },
  modalHeader: {
    flexDirection: "row",
    justifyContent: "space-between",
    paddingHorizontal: 20,
    paddingTop: 60,
    paddingBottom: 15,
  },
  buttonText: {
    fontSize: 17,
    fontFamily: typography.primary.normal,
  },
  cancelText: {
    color: "#666",
  },
  sendText: {
    color: "#fff",
  },
  disabledText: {
    color: "#666",
  },
  input: {
    color: "#fff",
    fontSize: 17,
    paddingHorizontal: 20,
    paddingTop: 0,
    fontFamily: typography.primary.normal,
  },

  // Chat Overlay styles
  chatOverlay: {
    position: "absolute",
    top: 40,
    left: 0,
    right: 0,
    bottom: 110,
    padding: 10,
    zIndex: 5,
    borderBottomWidth: 1,
    borderBottomColor: "rgba(255,255,255,0.1)",
  },
  messageList: {
    flex: 1,
  },
  message: {
    marginBottom: 12,
  },
  messageText: {
    color: "#fff",
    fontSize: 15,
    fontFamily: typography.primary.normal,
  },

  // Error styles
  errorContainer: {
    backgroundColor: colors.errorBackground,
    padding: 12,
    borderRadius: 8,
    marginBottom: 16,
  },
  errorText: {
    color: colors.error,
    fontSize: 14,
    fontFamily: typography.primary.normal,
  },
})
