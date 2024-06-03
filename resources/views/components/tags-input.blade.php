@props(['name', 'tags' => []])

<div x-data="tagsComponent()" class="flex flex-col">
    <input
        x-model="newTag"
        type="text"
        placeholder="Add a new tag..."
        class="px-4 py-2 my-2 border border-darkgray bg-black text-white rounded-md focus:outline-none focus:ring-2 focus:ring-white focus:border-white"
        @keyup="enterTag($event)"
        @blur="addTag(newTag)"
        @keydown.enter.stop
        @keyup.enter.stop
        @keydown.enter.prevent
        @keyup.enter.prevent
    >
    <div class="flex flex-wrap gap-2 mt-2">
        <template x-for="tag in tags" :key="tag">
            <div role="button" class="flex items-center px-3 py-1 rounded-md border border-[#cbcbcb] text-white hover:text-black hover:bg-white">
                <span x-text="tag"></span>
                <button
                    @click="removeTag(tag)"
                    @keydown.enter.stop
                    @keyup.enter.stop
                    @keydown.enter.prevent
                    @keyup.enter.prevent
                    tabindex="-1"
                    class="ml-2  hover:text-black focus:outline-none"
                >
                    <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"></path>
                    </svg>
                </button>
            </div>
        </template>
    </div>
</div>

<script>
function tagsComponent() {
    let initialTags =@json($tags);
    if(!Array.isArray(initialTags))  initialTags=[];

    return {
        newTag: '',
        tags: initialTags,
        addTag(newTag) {
            newTag=newTag.trim();
            if (newTag !== '') {
                if(!this.tags.includes(newTag)){
                    this.tags.push(newTag);
                }
                this.newTag = '';
            }
            this.$dispatch('tags-updated', [this.tags]);
        },
        removeTag(tag) {
            this.tags = this.tags.filter(t => t !== tag);
            this.$dispatch('tags-updated', [this.tags]);
        },
        enterTag(event) {
            if (event.key === ',' || event.code === 'Comma' || event.code === 'Enter') {
                if(event.key === ',' || event.code === 'Comma' ){
                    this.addTag(this.newTag.slice(0, -1));
                }else{
                    this.addTag(this.newTag);
                }
                event.preventDefault();
                event.stopPropagation();
            }
        }
    }
}
</script>
