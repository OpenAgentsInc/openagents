<x-blog-layout>
    <div class="flex">
        <div class="mx-auto">
            <div id="nostr-login-status">Loading...</div>

            <div id="nostr-login-try-again" class="hidden">
                <a href="/login">
                    <x-button>Back</x-button>
                </a>
                <a href="">
                    <x-button>Try Again</x-button>
                </a>
            </div>

            <form class="hidden" id="nostr-form" method="POST">
                @csrf
                <input id="nostr-form-event" type="hidden" name="event" value="null">
            </form>
        </div>
    </div>

    <script>
        document.onreadystatechange = async () => {
            setTimeout(async () => {
                const status = document.getElementById('nostr-login-status')

                function error(msg) {
                    status.innerHTML = `Error: ${msg}<br/>`;
                    document.getElementById("nostr-login-try-again").classList.remove("hidden");
                }

                try {

                    if (window.location.hash == "#error") {
                        error("server rejected")
                        return
                    }

                    status.innerHTML = "Authenticating with nostr extention...";
                    const pubkey = await window.nostr.getPublicKey();
                    const event = {
                        pubkey,
                        kind: 27235,
                        created_at: Math.round(Date.now() / 1000),
                        tags: [
                            ["u", "https://openagents.com/login/nostr"],
                            ["method", "POST"]
                        ],
                        content: "",
                    }
                    const se = await window.nostr.signEvent(event);

                    document.getElementById('nostr-form-event').value = btoa(JSON.stringify(se));
                    document.getElementById('nostr-form').submit()

                } catch (err) {
                    error(err.message);
                }
            }, 25)
        }
    </script>
</x-blog-layout>
