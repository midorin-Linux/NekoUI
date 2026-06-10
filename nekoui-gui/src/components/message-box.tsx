import { useRef } from "react";
import {Plus, Send, Image, Files, Camera} from "lucide-react";

import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import {Popover, PopoverContent, PopoverTrigger} from "@/components/ui/popover.tsx";
import {Separator} from "@/components/ui/separator.tsx";
import {Field, FieldLabel} from "@/components/ui/field.tsx";
import {Switch} from "@/components/ui/switch.tsx";

const placeholder = "Ask Anything...";

export default function MessageBox() {
    const textareaRef = useRef<HTMLTextAreaElement>(null);

    return (
        <div className="rounded-2xl border p-2 h-26 text-sm w-full bg-white hover:border-gray-300">
            <Textarea
                ref={textareaRef}
                placeholder={placeholder}
                className="border-none shadow-none focus-visible:ring-0 pb-0 resize-none min-h-10 max-h-50 overflow-y-auto"
            />

            <div className="flex justify-between">
                <div className="flex">
                    <Popover>
                        <PopoverTrigger asChild>
                            <Button variant="ghost" className="hover:bg-primary/10 size-9 rounded-xl">
                                <Plus />
                            </Button>
                        </PopoverTrigger>
                        <PopoverContent className="p-1.5 w-fit" align="start" side="top">
                            <section className="flex gap-2 justify-stretch h-fit">
                                <Button variant="outline" className="size-22 flex flex-col justify-center items-center bg-primary/2 hover:bg-primary/10">
                                    <Image strokeWidth="1" className="size-8" />
                                    <span className="text-xs font-light">Images</span>
                                </Button>
                                <Button variant="outline" className="size-22 flex flex-col justify-center items-center bg-primary/2 hover:bg-primary/10">
                                    <Files strokeWidth="1" className="size-8" />
                                    <span className="text-xs font-light">Files</span>
                                </Button>
                                <Button variant="outline" className="size-22 flex flex-col justify-center items-center bg-primary/2 hover:bg-primary/10">
                                    <Camera strokeWidth="1" className="size-8" />
                                    <span className="text-xs font-light">Take photos</span>
                                </Button>
                            </section>
                            <Separator />
                            <section className="flex flex-col gap-1">
                                <Field orientation="horizontal" className="w-full px-1.5 py-3">
                                    <FieldLabel>Web search</FieldLabel>
                                    <Switch />
                                </Field>
                            </section>
                        </PopoverContent>
                    </Popover>
                    {/*<DropdownMenu>*/}
                    {/*    <DropdownMenuTrigger asChild>*/}
                    {/*        <Button variant="ghost" className="hover:bg-primary/10 size-9 rounded-xl">*/}
                    {/*            <Plus />*/}
                    {/*        </Button>*/}
                    {/*    </DropdownMenuTrigger>*/}
                    {/*    <DropdownMenuContent className="w-70" align="start">*/}
                    {/*        <DropdownMenuGroup>*/}
                    {/*            <DropdownMenuItem>Settings</DropdownMenuItem>*/}
                    {/*            <DropdownMenuSub>*/}
                    {/*                <DropdownMenuSubTrigger>Language</DropdownMenuSubTrigger>*/}
                    {/*                <DropdownMenuPortal>*/}
                    {/*                    <DropdownMenuSubContent>*/}
                    {/*                        <DropdownMenuItem>English</DropdownMenuItem>*/}
                    {/*                        <DropdownMenuItem>日本語</DropdownMenuItem>*/}
                    {/*                    </DropdownMenuSubContent>*/}
                    {/*                </DropdownMenuPortal>*/}
                    {/*            </DropdownMenuSub>*/}
                    {/*            <DropdownMenuItem>Help</DropdownMenuItem>*/}
                    {/*        </DropdownMenuGroup>*/}
                    {/*        <DropdownMenuSeparator />*/}
                    {/*        <DropdownMenuGroup>*/}
                    {/*            <DropdownMenuItem>logout</DropdownMenuItem>*/}
                    {/*        </DropdownMenuGroup>*/}
                    {/*    </DropdownMenuContent>*/}
                    {/*</DropdownMenu>*/}
                </div>
                <div className="flex">
                    <Button className="size-9 rounded-xl" type="button">
                        <Send />
                    </Button>
                </div>
            </div>
        </div>
    );
}