# Everglade II familiar senses v1

`M7 SEQ` requests borrowed familiar senses over Everglade session v5.
The positive sequence increases across familiar requests. The room uses
the owner's saved Bonus Action and shared familiar link. Success
replies `m7 SEQ` to the owner. Refusal replies `r SEQ familiar`.

A separate owner-only `m8 MODE` record carries the current viewpoint.
Mode 0 uses the owner, mode 1 borrows the familiar's sight, and mode 2
borrows sight and hearing. The room sends changes and periodic refreshes.
The client also needs a visible row for the owner's familiar in the
separate position snapshot before it moves the camera or listener.
The shared link and start of the owner's next turn govern expiry.
