// Include the libwebsockets library
#include <libwebsockets.h>
// Include the string.h library for string functions
#include <string.h>
// Include the stdio.h library for input/output functions
#include <stdio.h>

// Define the URL of the WebSocket server
#define SERVER_URL "wss://relay.wellorder.net"

// Callback function for handling WebSocket events
static int callback_http(struct lws *wsi, enum lws_callback_reasons reason, void *user, void *in, size_t len)
{
    // Switch based on the reason for the callback
    switch (reason) {
        // Case when the WebSocket connection is established
        case LWS_CALLBACK_CLIENT_ESTABLISHED:
            // Print a message indicating connection establishment
            printf("WebSocket connection established\n");
            break;
        // Case when a message is received from the server
        case LWS_CALLBACK_CLIENT_RECEIVE:
            // Print the received message
            printf("Received message: %.*s\n", (int)len, (char *)in);
            break;
        // Case when there is a connection error
        case LWS_CALLBACK_CLIENT_CONNECTION_ERROR:
            // Print a message indicating a connection error
            printf("WebSocket connection error\n");
            break;
        // Default case for other reasons
        default:
            break;
    }

    // Return 0 to indicate successful handling of the callback
    return 0;
}

// Main function
int main(int argc, char **argv)
{
    // Struct to hold context creation information
    struct lws_context_creation_info info;
    // Struct to hold connection information
    struct lws_client_connect_info conn_info;
    // Pointer to the WebSocket context
    struct lws_context *context;
    // Pointer to the WebSocket connection
    struct lws *wsi;

    // Initialize the context creation information struct
    memset(&info, 0, sizeof(info));
    // Initialize the connection information struct
    memset(&conn_info, 0, sizeof(conn_info));

    // Set up the WebSocket context creation information
    info.port = CONTEXT_PORT_NO_LISTEN; // No listening port
    info.protocols = NULL; // No protocols
    info.gid = -1; // No group ID
    info.uid = -1; // No user ID

    lws_set_log_level(LLL_ERR | LLL_WARN | LLL_NOTICE | LLL_INFO | LLL_DEBUG, NULL);

    // Create the WebSocket context
    context = lws_create_context(&info);

    // Check if context creation failed
    if (!context) {
        // Print an error message
        printf("Failed to create WebSocket context\n");
        return 1; // Exit with an error code
    }

    // Set up the connection information
    conn_info.context = context; // Set the context
    conn_info.address = SERVER_URL; // Set the server URL
    conn_info.path = "/"; // Set the path
    conn_info.host = lws_canonical_hostname(context); // Set the host
    conn_info.origin = SERVER_URL; // Set the origin
    conn_info.protocol = NULL; // Set the protocol to NULL
    conn_info.ietf_version_or_minus_one = -1; // Set the WebSocket version
    conn_info.ssl_connection = LCCSCF_USE_SSL | LCCSCF_ALLOW_SELFSIGNED | LCCSCF_SKIP_SERVER_CERT_HOSTNAME_CHECK;


    // Connect to the WebSocket server
    wsi = lws_client_connect_via_info(&conn_info);
    if (!wsi) {
        printf("Failed to connect to WebSocket server\n");
        lws_context_destroy(context);
        return 1;
    }

    // Service the WebSocket connection
    while (1) {
        lws_service(context, 0); // Service the connection
        // ... add your custom logic here ...
    }

    // Clean up by destroying the WebSocket context
    lws_context_destroy(context);

    // Exit the program successfully
    return 0;
}
