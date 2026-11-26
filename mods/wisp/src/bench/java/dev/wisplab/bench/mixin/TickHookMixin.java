package dev.wisplab.bench.mixin;

import dev.wisplab.bench.VanillaBench;

import net.minecraft.client.Minecraft;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Times the client tick (Minecraft.tick), for the stress scenes' client tick figure. */
@Mixin(Minecraft.class)
public abstract class TickHookMixin {
    @Inject(method = "tick", at = @At("HEAD"))
    private void wispBenchTickStart(CallbackInfo callback) {
        VanillaBench.tick(true);
    }

    @Inject(method = "tick", at = @At("RETURN"))
    private void wispBenchTickEnd(CallbackInfo callback) {
        VanillaBench.tick(false);
    }
}
