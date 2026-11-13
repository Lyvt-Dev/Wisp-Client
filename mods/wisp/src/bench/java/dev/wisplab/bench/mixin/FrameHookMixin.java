package dev.wisplab.bench.mixin;

import dev.wisplab.bench.VanillaBench;

import net.minecraft.client.Minecraft;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(Minecraft.class)
public abstract class FrameHookMixin {
    @Inject(method = "runTick", at = @At("HEAD"))
    private void wispBenchFrame(boolean advanceGameTime, CallbackInfo callback) {
        VanillaBench.frame((Minecraft) (Object) this);
    }
}
